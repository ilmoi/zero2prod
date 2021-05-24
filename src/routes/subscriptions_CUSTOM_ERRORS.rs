use std::convert::TryInto;

use actix_web::{web, HttpResponse, ResponseError};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;
use actix_web::http::StatusCode;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

impl TryInto<NewSubscriber> for FormData {
    type Error = String;
    fn try_into(self) -> Result<NewSubscriber, Self::Error> {
        let name = SubscriberName::parse(String::from(self.name))?;
        let email = SubscriberEmail::parse(String::from(self.email))?;
        Ok(NewSubscriber { email, name })
    }
}

// -----------------------------------------------------------------------------
// main function

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pg_pool, email_client, base_url),
    fields(
        // request_id = %Uuid::new_v4(), //we no longer want this after we've added TracingLogger to run()
        email = %form.email,
        name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pg_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
    //return actix_web type error
) -> Result<HttpResponse, actix_web::Error> {
    // ValidationError(String),
    let new_subscriber = form
        .0
        .try_into()
        .map_err(|e| SubscribeError::ValidationError(ValidationError(e)))?;

    // ####################### DATABASE ###########################
    // PoolError(sqlx::Error),
    let mut transaction = pg_pool
        .begin()
        .await
        .map_err(|e| SubscribeError::PoolError(PoolError(e)))?;

    // InsertSubscriberError(sqlx::Error),
    let sub_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .map_err(|e| SubscribeError::InsertSubscriberError(InsertSubscriberError(e)))?;

    // StoreTokenError(sqlx::Error),
    let sub_token = gen_sub_token();
    store_token(&mut transaction, sub_id, &sub_token)
        .await
        .map_err(|e| SubscribeError::StoreTokenError(StoreTokenError(e)))?;

    // TransactionCommitError(sqlx::Error),
    transaction
        .commit()
        .await
        .map_err(|e| SubscribeError::TransactionCommitError(TransactionCommitError(e)))?;
    // ##############################################################

    // SendEmailError(reqwest::Error),
    send_confirmation_email(&email_client, new_subscriber, &base_url.0, &sub_token)
        .await
        .map_err(|e| SubscribeError::SendEmailError(SendEmailError(e)))?;

    Ok(HttpResponse::Ok().finish())
}

// -----------------------------------------------------------------------------
// helper functions

#[tracing::instrument(
    name = "sending confirmation email",
    skip(email_client, new_subscriber, base_url, sub_token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    sub_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!("{}/subscriptions/confirm?sub_token={}", base_url, sub_token);
    let plain_body = format!(
        "Welcome to our newsletter!<br />\
        Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );
    let html_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    email_client
        .send_email(new_subscriber.email, "Welcome!", &html_body, &plain_body)
        .await?;
    Ok(())
}

#[tracing::instrument(
    name = "Saving new subscriber details in the db",
    skip(transaction, new_sub)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_sub: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let sub_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        sub_id,
        new_sub.email.as_ref(),
        new_sub.name.as_ref(),
        Utc::now()
    )
    .execute(transaction)
    .await?;
    Ok(sub_id)
}

fn gen_sub_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "insert into sub_tokens table",
    skip(transaction, sub_id, sub_token)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    sub_id: Uuid,
    sub_token: &String,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscription_tokens (sub_token, sub_id)
        VALUES ($1, $2)
        "#,
        sub_token,
        sub_id,
    )
    .execute(transaction)
    .await?;
    Ok(())
}

// -----------------------------------------------------------------------------
// errors

// 1) convert sqlx::Error into our own type
#[derive(Debug)]
pub struct ValidationError(String);
pub struct StoreTokenError(sqlx::Error);
pub struct InsertSubscriberError(sqlx::Error);
pub struct PoolError(sqlx::Error);
pub struct TransactionCommitError(sqlx::Error);
pub struct SendEmailError(reqwest::Error);
pub enum SubscribeError {
    ValidationError(ValidationError),
    StoreTokenError(StoreTokenError),
    PoolError(PoolError),
    InsertSubscriberError(InsertSubscriberError),
    TransactionCommitError(TransactionCommitError),
    SendEmailError(SendEmailError),
}

// 2) implement ResponseError trait to be able to convert it into an actix_web type error and return as part of HttpResponse
impl ResponseError for ValidationError {}
impl ResponseError for StoreTokenError {}
impl ResponseError for InsertSubscriberError {}
impl ResponseError for PoolError {}
impl ResponseError for TransactionCommitError {}
impl ResponseError for SendEmailError {}
impl ResponseError for SubscribeError {
    //todo this fn is basically the entire reason we implemented the extra class on top - we wanted control over status code specifically for subscribe endpoint
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// 3) implement display + debug
impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "A validation error occured on the input.")
    }
}
impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a sub token."
        )
    }
}
impl std::fmt::Display for InsertSubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to insert a asub."
        )
    }
}
impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to start a pool."
        )
    }
}
impl std::fmt::Display for TransactionCommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to commit a tx."
        )
    }
}
impl std::fmt::Display for SendEmailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to send an email.")
    }
}
impl std::fmt::Display for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to create a subscriber.")
    }
}

// impl std::fmt::Debug for ValidationError { //todo can only impl for <dyn Error>
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         error_chain_fmt(self, f)
//     }
// }
impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl std::fmt::Debug for InsertSubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl std::fmt::Debug for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl std::fmt::Debug for TransactionCommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl std::fmt::Debug for SendEmailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

// 4) implement Error trait - this allows it to point to other Error trait objects as the source
// impl std::error::Error for ValidationError { //todo can only impl for <dyn Error> and string isn't that
//     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
//         Some(&self.0)
//     }
// }
impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
impl std::error::Error for InsertSubscriberError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
impl std::error::Error for PoolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
impl std::error::Error for TransactionCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
impl std::error::Error for SendEmailError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
impl std::error::Error for SubscribeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SubscribeError::ValidationError(e) => None, //todo forced to return none here as String doesn't impl <dyn Error>
            SubscribeError::StoreTokenError(e) => Some(e),
            SubscribeError::InsertSubscriberError(e) => Some(e),
            SubscribeError::PoolError(e) => Some(e),
            SubscribeError::TransactionCommitError(e) => Some(e),
            SubscribeError::SendEmailError(e) => Some(e),
        }
    }
}

// -----------------------------------------------------------------------------
// error helpers

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by: \n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
