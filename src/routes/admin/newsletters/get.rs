use actix_web::{error::ErrorInternalServerError, http::header::ContentType, web, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use anyhow::Context;
use sqlx::PgPool;
use tera::Tera;

use crate::{authentication::UserId, routes::get_username, utils::e500};

pub async fn send_newsletter_form(
    tera: web::Data<Tera>,
    db_pool: web::Data<PgPool>,
    user_id: web::ReqData<UserId>,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let username = get_username(*user_id, &db_pool).await.map_err(e500)?;

    #[derive(serde::Serialize)]
    struct BodyData {
        messages: Vec<String>,
        username: String,
    }

    let messages: Vec<String> = flash_messages
        .iter()
        .map(|m| m.content().to_string())
        .collect();

    let body_data = BodyData { messages, username };

    let render_context = tera::Context::from_serialize(body_data)
        .context("Failed to build context")
        .map_err(e500)?;

    let body = tera
        .render("admin/send-newsletter.j2", &render_context)
        .context("Failed to render send newsletter")
        .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}
