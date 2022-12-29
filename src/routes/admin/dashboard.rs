use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use tera::Tera;
use uuid::Uuid;

use crate::{authentication::UserId, utils::e500};

#[derive(serde::Serialize)]
struct DashboardContext {
    username: String,
}

#[tracing::instrument(name = "Get admin dashboard", skip(db_pool, user_id, tera))]
pub async fn admin_dashboard(
    db_pool: web::Data<PgPool>,
    user_id: web::ReqData<UserId>,
    tera: web::Data<Tera>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let username = get_username(*user_id, &db_pool).await.map_err(e500)?;

    let body = {
        let context = tera::Context::from_serialize(DashboardContext { username })
            .context("Failed to serialize context")
            .map_err(e500)?;

        tera.render("admin/dashboard.j2", &context)
            .context("Failed to render admin dashboard")
            .map_err(e500)?
    };

    tracing::debug!("body={}", body);

    Ok(HttpResponse::Ok().body(body))
}

#[tracing::instrument(name = "Get username", skip(db_pool))]
pub async fn get_username(user_id: Uuid, db_pool: &PgPool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!("SELECT username FROM users WHERE user_id = $1", user_id)
        .fetch_one(db_pool)
        .await
        .context("Failed to perform a query to retrieve a username.")?;
    Ok(row.username)
}
