use std::fmt::Debug;

use actix_web::{cookie::Cookie, http::header::ContentType, web, HttpResponse, ResponseError};
use actix_web_flash_messages::{IncomingFlashMessages, Level};
use anyhow::Context;
use tera::Tera;

use crate::routes::error_chain_fmt;

#[derive(serde::Serialize)]
pub struct LoginData {
    error_message: Option<String>,
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for LoginError {
    fn error_response(&self) -> HttpResponse {
        match self {
            LoginError::UnexpectedError(_) => HttpResponse::InternalServerError().finish(),
        }
    }
}

pub async fn login_form(
    flash_messages: IncomingFlashMessages,
    tera: web::Data<Tera>,
) -> Result<HttpResponse, LoginError> {
    let body = {
        let error_message = flash_messages.iter().find(|m| m.level() == Level::Error);
        let login_data = LoginData {
            error_message: match error_message {
                None => None,
                Some(e) => Some(e.content().to_string()),
            },
        };
        let context =
            tera::Context::from_serialize(&login_data).context("Failed to serialize context")?;
        tera.render("login.html", &context)
            .context("Failed to render html body")?
    };

    let mut response = HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body);

    response
        .add_removal_cookie(&Cookie::new("_flash", ""))
        .context("failed to add removal cookie")?;

    Ok(response)
}
