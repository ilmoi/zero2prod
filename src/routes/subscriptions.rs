use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

pub async fn subscribe(
    form: web::Form<FormData>,
    // "dependency injection"
    // connection: web::Data<Arc<PgConnection>>, //Data is an extractor - extracts whatever is stored under type <Arc<PgConnection>> in data
    pg_pool: web::Data<PgPool>,
) -> Result<HttpResponse, HttpResponse> {
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
    // web::Data<Arc<PgConnection>> is equivalent to Arc<Arc<PgConnection>>
    // so to get it we first do get_ref >  &Arc<PgConnection>, then deref() to get &PgConnection
    // .deref() - discussed here https://doc.rust-lang.org/stable/book/ch15-02-deref.html - anything that has deref implemented on it can be used to extract the inner something. &Arc<something> -> &something
    // .get_ref() - seems to be specific to actix, I couldn't find it in general docs - https://docs.rs/actix-web/4.0.0-beta.3/actix_web/web/struct.Data.html#method.get_ref
    // .execute(connection.get_ref().deref())
    // this time with pg_pool we only unwrap once
    .execute(pg_pool.get_ref())
    .await
    //map_err - coerces one type of error into another by applying a function (in this case closure) to it -https://doc.rust-lang.org/std/result/enum.Result.html#method.map_err
    .map_err(|e| {
        println!("failed to execute query {}", e);
        HttpResponse::InternalServerError().finish()
    })?;
    Ok(HttpResponse::Ok().finish())
}
