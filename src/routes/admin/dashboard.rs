use actix_web::{web, HttpResponse};
use anyhow::Context;
use reqwest::header::LOCATION;
use sqlx::PgPool;
use tera::Tera;
use uuid::Uuid;

use crate::{session_state::TypedSession, utils::e500};

#[derive(serde::Serialize)]
struct DashboardContext {
    username: String,
}

#[tracing::instrument(name = "Get admin dashboard", skip(db_pool, session, tera))]
pub async fn admin_dashboard(
    db_pool: web::Data<PgPool>,
    session: TypedSession,
    tera: web::Data<Tera>,
) -> Result<HttpResponse, actix_web::Error> {
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        get_username(user_id, &db_pool).await.map_err(e500)?
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };

    let body = {
        let context = tera::Context::from_serialize(DashboardContext { username })
            .context("Failed to serialize context")
            .map_err(e500)?;

        tera.render("admin/dashboard.html", &context)
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
