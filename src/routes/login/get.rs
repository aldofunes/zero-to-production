use std::fmt::Debug;

use actix_web::{http::header::ContentType, web, HttpResponse, ResponseError};
use anyhow::Context;
use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use tera::Tera;

use crate::{routes::error_chain_fmt, startup::HmacSecret};

#[derive(serde::Deserialize)]
pub struct QueryParams {
    error: String,
    tag: String,
}

impl QueryParams {
    fn verify(self, hmac_secret: &HmacSecret) -> Result<String, anyhow::Error> {
        let tag = hex::decode(self.tag)?;
        let query_string = format!("error={}", urlencoding::Encoded::new(&self.error));

        let mut mac =
            Hmac::<sha2::Sha256>::new_from_slice(hmac_secret.0.expose_secret().as_bytes()).unwrap();
        mac.update(query_string.as_bytes());
        mac.verify_slice(&tag)?;

        Ok(self.error)
    }
}

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
    query: Option<web::Query<QueryParams>>,
    tera: web::Data<Tera>,
    hmac_secret: web::Data<HmacSecret>,
) -> Result<HttpResponse, LoginError> {
    let body = {
        let login_data = match query {
            None => LoginData {
                error_message: None,
            },
            Some(query) => match query.0.verify(&hmac_secret) {
                Ok(error_message) => LoginData {
                    error_message: Some(error_message),
                },
                Err(e) => {
                    tracing::warn!(
                        error.message = %e,
                        error.cause_chain = ?e,
                        "Failed to verify query parameters using the HMAC tag"
                    );
                    LoginData {
                        error_message: None,
                    }
                }
            },
        };
        let context =
            tera::Context::from_serialize(&login_data).context("Failed to serialize context")?;
        tera.render("login.html", &context)
            .context("Failed to render html body")?
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}
