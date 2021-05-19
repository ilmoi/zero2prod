use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

//we can take out all the tracing logic out of the function and write it out separately, here
//what this does is:
//1)creates a span in the beginning of function invocation
//2)automatically attaches all the fn args to the span (but we can tell tracing to ignore = "skip" them)
//3)we can manually enrich the span's context with field directive
//4)automatically uses tracing-futures if applied to an async function
#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pg_pool),
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
) -> Result<HttpResponse, HttpResponse> {
    insert_subscriber(&pg_pool, &form)
        .await
        .map_err(|_| HttpResponse::InternalServerError().finish())?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Saving new subscriber details in the db", skip(form, pg_pool))]
// takes care of the database logic and has no awareness of the surrounding web framework
pub async fn insert_subscriber(pg_pool: &PgPool, form: &FormData) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pg_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
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
