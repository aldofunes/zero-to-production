use crate::{
    authentication::reject_anonymous_users,
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{
        admin_dashboard, change_password, change_password_form, confirm, health_check, home,
        log_out, login, login_form, publish_newsletter, send_newsletter_form, subscribe,
    },
    tera::init_tera,
};
use actix_files::Files;
use actix_session::{storage::RedisSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, dev::Server, web, App, HttpServer};
use actix_web_flash_messages::{storage::CookieMessageStore, FlashMessagesFramework};
use actix_web_lab::middleware::from_fn;
use secrecy::{ExposeSecret, Secret};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{net::TcpListener, time::Duration};
use tera::Tera;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        let tera = init_tera();

        let db_pool = get_db_pool(&configuration.database);
        let email_client = configuration.email_client.client();

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
            tera,
            configuration.application.base_url,
            configuration.application.hmac_secret,
            configuration.redis_uri,
        )
        .await?;

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

async fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    tera: Tera,
    base_url: String,
    hmac_secret: Secret<String>,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
    // wrap the connection in a smart pointer
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));
    let tera = web::Data::new(tera);

    let secret_key = Key::from(hmac_secret.expose_secret().as_bytes());
    let mesage_store = CookieMessageStore::builder(secret_key.clone()).build();
    let message_framework = FlashMessagesFramework::builder(mesage_store).build();
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;
    let server = HttpServer::new(move || {
        App::new()
            .service(Files::new("/static", ".").show_files_listing())
            .wrap(message_framework.clone())
            .wrap(SessionMiddleware::new(
                redis_store.clone(),
                secret_key.clone(),
            ))
            .wrap(TracingLogger::default())
            .route("/", web::get().to(home))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .service(
                web::scope("/admin")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/dashboard", web::get().to(admin_dashboard))
                    .route("/password", web::get().to(change_password_form))
                    .route("/password", web::post().to(change_password))
                    .route("/logout", web::post().to(log_out))
                    .route("/newsletters", web::post().to(publish_newsletter))
                    .route("/newsletters", web::get().to(send_newsletter_form)),
            )
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(tera.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
