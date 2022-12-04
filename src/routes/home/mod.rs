use std::fmt::Debug;

use actix_web::{http::header::ContentType, web, HttpResponse, ResponseError};
use anyhow::Context;
use tera::Tera;

use super::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum HomeError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for HomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for HomeError {
    fn error_response(&self) -> HttpResponse {
        match self {
            HomeError::UnexpectedError(_) => HttpResponse::InternalServerError().finish(),
        }
    }
}

pub async fn home(tera: web::Data<Tera>) -> Result<HttpResponse, HomeError> {
    let context = tera::Context::new();
    let body = tera
        .render("index.html", &context)
        .context("Failed to render html body")?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}
