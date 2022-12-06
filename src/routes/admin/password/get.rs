use actix_web::{error::ErrorInternalServerError, http::header::ContentType, web, HttpResponse};
use anyhow::Context;
use tera::Tera;

pub async fn change_password_form(tera: web::Data<Tera>) -> Result<HttpResponse, actix_web::Error> {
    let body = tera
        .render("admin/change_password.html", &tera::Context::new())
        .context("Failed to render change password")
        .map_err(|e| ErrorInternalServerError(e))?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}
