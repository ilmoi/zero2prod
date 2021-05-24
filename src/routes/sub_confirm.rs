use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    sub_token: String,
}

#[tracing::instrument(name = "confirm a pending subscriber", skip(params))]
pub async fn confirm(
    params: web::Query<Parameters>,
    pg_pool: web::Data<PgPool>,
) -> Result<HttpResponse, HttpResponse> {
    let id = get_sub_id_from_token(&pg_pool, &params.sub_token)
        .await
        .map_err(|_| HttpResponse::InternalServerError().finish())?;

    match id {
        None => Err(HttpResponse::Unauthorized().finish()),
        Some(sub_id) => {
            confirm_sub(&pg_pool, sub_id)
                .await
                .map_err(|_| HttpResponse::InternalServerError().finish())?;
            Ok(HttpResponse::Ok().finish())
        }
    }
}

#[tracing::instrument(name = "find matching record in db", skip(pg_pool, sub_token))]
pub async fn get_sub_id_from_token(
    pg_pool: &PgPool,
    sub_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"SELECT sub_id FROM subscription_tokens WHERE sub_token = $1
        "#,
        sub_token
    )
    .fetch_optional(pg_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(result.map(|r| r.sub_id))
}

#[tracing::instrument(name = "mark confirmed", skip(pg_pool, sub_id))]
pub async fn confirm_sub(pg_pool: &PgPool, sub_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions
        SET status = 'confirmed'
        WHERE id = $1
        "#,
        sub_id
    )
    .execute(pg_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}
