use crate::routes::FormData;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

#[tracing::instrument(
    name = "Renaming user",
    skip(form, pg_pool),
    fields(
        email = %form.email,
        name = %form.name
    )
)]
pub async fn rename(
    form: web::Form<FormData>,
    pg_pool: web::Data<PgPool>,
) -> Result<HttpResponse, HttpResponse> {
    internal_rename(&pg_pool, &form).await.map_err(|e| {
        tracing::error!("failed to rename {:?}", e);
        HttpResponse::InternalServerError().finish()
    })?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Updating db", skip(form, pg_pool))]
async fn internal_rename(pg_pool: &PgPool, form: &FormData) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions
        SET name = $1
        WHERE email = $2
        "#,
        form.name,
        form.email
    )
    .execute(pg_pool)
    .await
    .map_err(|e| {
        tracing::error!("failed to exec query {:?}", e);
        e
    })?;
    Ok(())
}
