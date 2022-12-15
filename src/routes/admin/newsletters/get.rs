use actix_web::{error::ErrorInternalServerError, http::header::ContentType, web, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use anyhow::Context;
use tera::Tera;

use crate::utils::e500;

pub async fn send_newsletter_form(
    tera: web::Data<Tera>,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    #[derive(serde::Serialize)]
    struct BodyData {
        messages: Vec<String>,
    }

    let messages: Vec<String> = flash_messages
        .iter()
        .map(|m| m.content().to_string())
        .collect();

    let body_data = BodyData { messages };

    let render_context = tera::Context::from_serialize(body_data)
        .context("Failed to build context")
        .map_err(e500)?;

    let body = tera
        .render("admin/send-newsletter.html", &render_context)
        .context("Failed to render send newsletter")
        .map_err(|e| ErrorInternalServerError(e))?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}
