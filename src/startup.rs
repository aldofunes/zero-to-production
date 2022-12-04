use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{confirm, health_check, home, login, login_form, publish_newsletter, subscribe},
    tera::init_tera,
};
use actix_web::{dev::Server, web, App, HttpServer};
use secrecy::Secret;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{net::TcpListener, time::Duration};
use tera::Tera;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let tera = init_tera();

        let db_pool = get_db_pool(&configuration.database);

        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address.");
        let timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
            timeout,
        );

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        tracing::info!("Starting listener at {}", address);
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            db_pool,
            email_client,
            configuration.application.base_url,
            tera,
            configuration.application.hmac_secret,
        )?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_db_pool(configuration: &DatabaseSettings) -> sqlx::Pool<sqlx::Postgres> {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy_with(configuration.with_db())
}

pub struct ApplicationBaseUrl(pub String);
pub struct HmacSecret(pub Secret<String>);

fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
    tera: Tera,
    hmac_secret: Secret<String>,
) -> Result<Server, std::io::Error> {
    // wrap the connection in a smart pointer
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));
    let tera = web::Data::new(tera);
    let hmac_secret = web::Data::new(HmacSecret(hmac_secret));

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/", web::get().to(home))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(tera.clone())
            .app_data(hmac_secret.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
