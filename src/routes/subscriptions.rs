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

//we could have a separate function for this, but this is an "accepted" way of doing conversions
impl TryInto<NewSubscriber> for FormData {
    type Error = String;
    fn try_into(self) -> Result<NewSubscriber, Self::Error> {
        let name = SubscriberName::parse(String::from(self.name))?;
        let email = SubscriberEmail::parse(String::from(self.email))?;
        Ok(NewSubscriber { email, name })
    }
}

//we can take out all the tracing logic out of the function and write it out separately, here
//what this does is:
//1)creates a span in the beginning of function invocation
//2)automatically attaches all the fn args to the span (but we can tell tracing to ignore = "skip" them)
//3)we can manually enrich the span's context with field directive
//4)automatically uses tracing-futures if applied to an async function
#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pg_pool, email_client, base_url),
    fields(
        // request_id = %Uuid::new_v4(), //we no longer want this after we've added TracingLogger to run()
        email = %form.email,
        name = %form.name
    )
)]
//orchestrates the work to be done by calling required routines and converts into HTTP responses
pub async fn subscribe(
    form: web::Form<FormData>,
    pg_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    //create subscriber
    let new_subscriber = form.0.try_into()?;
    // .map_err(|_| HttpResponse::BadRequest().finish())?;

    let mut transaction = pg_pool.begin().await.map_err(SubscribeError::PoolError)?;

    //insert them into db
    let sub_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .map_err(SubscribeError::InsertSubscriberError)?;

    //gen & store the token
    let sub_token = gen_sub_token();
    store_token(&mut transaction, sub_id, &sub_token).await?;
    // .map_err(|_| HttpResponse::InternalServerError().finish())?;

    //commit the transaction
    transaction
        .commit()
        .await
        .map_err(SubscribeError::TransactionCommitError)?;

    //send them an email
    // let base_url = base_url.get_ref().deref(); //todo not sure why we're not doing this?
    send_confirmation_email(&email_client, new_subscriber, &base_url.0, &sub_token).await?;
    // .map_err(|_| HttpResponse::InternalServerError().finish())?;

    Ok(HttpResponse::Ok().finish())
}

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
        .await
        .map_err(|e| {
            // todo how do I capture response body here?
            tracing::error!("Failed to send an email: {:?}", e);
            println!("error is: {:?}", e);
            e
        })
}

#[tracing::instrument(
    name = "Saving new subscriber details in the db",
    skip(transaction, new_sub)
)]
// takes care of the database logic and has no awareness of the surrounding web framework
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
    .await
    .map_err(|e| {
        // remove coz using "?" - and we should log errors when we handle them
        // tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(sub_id)
}

//generate a pseudorandom string of 25 alphanumeric chars
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
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"
        INSERT INTO subscription_tokens (sub_token, sub_id)
        VALUES ($1, $2)
        "#,
        sub_token,
        sub_id,
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        // remove coz using "?" - and we should log errors when we handle them
        // tracing::error!("Failed to execute query: {:?}", e);
        StoreTokenError(e) //note how we have to wrap the error in our special type for it to work
    })?;
    Ok(())
}

// -----------------------------------------------------------------------------

//this is a wrapper around sqlx::Error - so that we can impl a foreign trait on it (orphan rule)
// #[derive(Debug)]
pub struct StoreTokenError(sqlx::Error);

//this is a standard error implementation
impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

//we need to implement both Debug and Display - but we can't derive Display
impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a sub token."
        )
    }
}

// //instead of deriving it and getting the default impl, we decided to write our own to make the relship between StoreTokenError and slqx::error more explicit
impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // write!(f, "{}\nCaused by:\n\t{}", self, self.0)
        error_chain_fmt(self, f) //recursive way of doing it
    }
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?; //write the current error followed by empty line
    let mut current = e.source(); //grab the source
    while let Some(cause) = current {
        //check source exists
        writeln!(f, "Caused by: \n\t{}", cause)?; //if it does, write it as cause
        current = cause.source(); //grab its source next
    } //etc in a  loop
    Ok(())
}

// implement the foreign trait on it
// impl ResponseError for StoreTokenError {} //REMOVING this because we're going to be creating another custom error type specifically for subscribe endpoint below

// -----------------------------------------------------------------------------

// a custom error class purely for subscribe, not to mix concerns wiht other endpoints (they may wnat to display errors differently)
// overall we're doing 2 things:
// 1)preparing a ResponseError for the api
// 2)provider relevant diagnostic (source, debug, display) for the human

// ---------------------------
//MANUAL APPROACH

// // by using an enum + from we can get rid of all the map_err in our code
// // #[derive(Debug)]
// pub enum SubscribeError {
//     ValidationError(String),
//     // DatabaseError(sqlx::Error),
//     StoreTokenError(StoreTokenError),
//     SendEmailError(reqwest::Error),
//     PoolError(sqlx::Error),
//     InsertSubscriberError(sqlx::Error),
//     TransactionCommitError(sqlx::Error),
// }
//
// impl From<reqwest::Error> for SubscribeError {
//     fn from(e: reqwest::Error) -> Self {
//         Self::SendEmailError(e)
//     }
// }
// // impl From<sqlx::Error> for SubscribeError {
// //     fn from(e: sqlx::Error) -> Self {
// //         Self::DatabaseError(e)
// //     }
// // }
// impl From<StoreTokenError> for SubscribeError {
//     fn from(e: StoreTokenError) -> Self {
//         Self::StoreTokenError(e)
//     }
// }
// impl From<String> for SubscribeError {
//     fn from(e: String) -> Self {
//         Self::ValidationError(e)
//     }
// }
//
// impl std::error::Error for SubscribeError {}
//
// impl std::fmt::Display for SubscribeError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             SubscribeError::ValidationError(_) => write!(f, "Failed to validate input"),
//             SubscribeError::StoreTokenError(_) => write!(
//                 f,
//                 "Failed to store the confirmation token for a new subscriber."
//             ),
//             SubscribeError::SendEmailError(_) => {
//                 write!(f, "Failed to send a confirmation email.")
//             }
//             SubscribeError::PoolError(_) => {
//                 write!(f, "Failed to acquire a Postgres connection from the pool")
//             }
//             SubscribeError::InsertSubscriberError(_) => {
//                 write!(f, "Failed to insert new subscriber in the database.")
//             }
//             SubscribeError::TransactionCommitError(_) => {
//                 write!(
//                     f,
//                     "Failed to commit SQL transaction to store a new subscriber."
//                 )
//             }
//         }
//     }
// }
//
// impl std::fmt::Debug for SubscribeError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         error_chain_fmt(self, f)
//     }
// }
//
// impl ResponseError for SubscribeError {
//     fn status_code(&self) -> StatusCode {
//         match self {
//             SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
//             SubscribeError::PoolError(_)
//             | SubscribeError::TransactionCommitError(_)
//             | SubscribeError::InsertSubscriberError(_)
//             | SubscribeError::StoreTokenError(_)
//             | SubscribeError::SendEmailError(_) => StatusCode::INTERNAL_SERVER_ERROR,
//         }
//     }
// }

// ---------------------------
// AUTOMATIC USING A MACRO

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String), //string doesn't implement the Error trait, therefore it can't be returned in Error:source

    #[error("Failed to acquire a Postgres connection from the pool")]
    PoolError(#[source] sqlx::Error),

    #[error("Failed to insert new subscriber in the database.")]
    InsertSubscriberError(#[source] sqlx::Error),

    #[error("Failed to store the confirmation token for a new subscriber.")]
    StoreTokenError(#[from] StoreTokenError), //from actually = from + source

    #[error("Failed to commit SQL transaction to store a new subscriber.")]
    TransactionCommitError(#[source] sqlx::Error),

    #[error("Failed to send a confirmation email.")]
    SendEmailError(#[from] reqwest::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl From<String> for SubscribeError {
    fn from(e: String) -> Self {
        Self::ValidationError(e)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::PoolError(_)
            | SubscribeError::TransactionCommitError(_)
            | SubscribeError::InsertSubscriberError(_)
            | SubscribeError::StoreTokenError(_)
            | SubscribeError::SendEmailError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

//subscribe before we re-instrumented tracing
// pub async fn subscribe(
//     form: web::Form<FormData>,
//     // "dependency injection"
//     // connection: web::Data<Arc<PgConnection>>, //Data is an extractor - extracts whatever is stored under type <Arc<PgConnection>> in data
//     pg_pool: web::Data<PgPool>,
// ) -> Result<HttpResponse, HttpResponse> {
//     let request_id = Uuid::new_v4();
//     //spans like logs have an associated level
//     let request_span = tracing::info_span!(
//         "Adding new subscriber",
//         //we're adding strucutre info
//         //using % to tell tracing to use their Display impl for logging purposes
//         %request_id, //implicit naming - use variable name for its key
//         email = %form.email,
//         name = %form.name
//     );
//     //not enough to create the span, we also need to enter it
//     //manual way below, but that's not how we want to do it. we want to use Instrument, so that span auto opens/closes on async actions
//     let _request_span_guard = request_span.enter();
//
//     let query_span = tracing::info_span!("Saving new subscriber to db.");
//
//     sqlx::query!(
//         r#"
//         INSERT INTO subscriptions (id, email, name, subscribed_at)
//         VALUES ($1, $2, $3, $4)
//         "#,
//         Uuid::new_v4(),
//         form.email,
//         form.name,
//         Utc::now()
//     )
//     // web::Data<Arc<PgConnection>> is equivalent to Arc<Arc<PgConnection>>
//     // so to get it we first do get_ref >  &Arc<PgConnection>, then deref() to get &PgConnection
//     // .deref() - discussed here https://doc.rust-lang.org/stable/book/ch15-02-deref.html - anything that has deref implemented on it can be used to extract the inner something. &Arc<something> -> &something
//     // .get_ref() - seems to be specific to actix, I couldn't find it in general docs - https://docs.rs/actix-web/4.0.0-beta.3/actix_web/web/struct.Data.html#method.get_ref
//     // .execute(connection.get_ref().deref())
//     // this time with pg_pool we only unwrap once
//     .execute(pg_pool.get_ref())
//     .instrument(query_span) //exits the span every time the future is parked
//     .await
//     //map_err - coerces one type of error into another by applying a function (in this case closure) to it -https://doc.rust-lang.org/std/result/enum.Result.html#method.map_err
//     .map_err(|e| {
//         tracing::error!(
//             "request_id: {}, failed to execute query {:?}",
//             request_id,
//             e
//         );
//         HttpResponse::InternalServerError().finish()
//     })?;
//     // tracing::info!(
//     //     "done saving new subscribed to db, request_id: {}",
//     //     request_id
//     // );
//
//     Ok(HttpResponse::Ok().finish())
// }

//using standard logging instead of tracing
// pub async fn subscribe(
//     form: web::Form<FormData>,
//     // "dependency injection"
//     // connection: web::Data<Arc<PgConnection>>, //Data is an extractor - extracts whatever is stored under type <Arc<PgConnection>> in data
//     pg_pool: web::Data<PgPool>,
// ) -> Result<HttpResponse, HttpResponse> {
//     let request_id = Uuid::new_v4();
//     log::info!(
//         ">> request_id: {}, saving {}, {} as new subscriber to db",
//         request_id,
//         form.email,
//         form.name
//     );
//     sqlx::query!(
//         r#"
//         INSERT INTO subscriptions (id, email, name, subscribed_at)
//         VALUES ($1, $2, $3, $4)
//         "#,
//         Uuid::new_v4(),
//         form.email,
//         form.name,
//         Utc::now()
//     )
//     // web::Data<Arc<PgConnection>> is equivalent to Arc<Arc<PgConnection>>
//     // so to get it we first do get_ref >  &Arc<PgConnection>, then deref() to get &PgConnection
//     // .deref() - discussed here https://doc.rust-lang.org/stable/book/ch15-02-deref.html - anything that has deref implemented on it can be used to extract the inner something. &Arc<something> -> &something
//     // .get_ref() - seems to be specific to actix, I couldn't find it in general docs - https://docs.rs/actix-web/4.0.0-beta.3/actix_web/web/struct.Data.html#method.get_ref
//     // .execute(connection.get_ref().deref())
//     // this time with pg_pool we only unwrap once
//     .execute(pg_pool.get_ref())
//     .await
//     //map_err - coerces one type of error into another by applying a function (in this case closure) to it -https://doc.rust-lang.org/std/result/enum.Result.html#method.map_err
//     .map_err(|e| {
//         log::error!(
//             ">> request_id: {}, failed to execute query {:?}",
//             request_id,
//             e
//         );
//         HttpResponse::InternalServerError().finish()
//     })?;
//     log::info!(
//         ">> done saving new subscribed to db, request_id: {}",
//         request_id
//     );
//
//     Ok(HttpResponse::Ok().finish())
// }
