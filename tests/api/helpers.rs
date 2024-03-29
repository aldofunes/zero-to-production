use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, PasswordHasher, Version};
use once_cell::sync::Lazy;
use rand::{
    rngs::{OsRng, StdRng},
    Rng, SeedableRng,
};
use reqwest::Url;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use tera::Tera;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
    email_client::EmailClient,
    issue_delivery_worker::{try_execute_task, ExecutionOutcome},
    startup::{get_db_pool, Application},
    telemetry::{get_subscriber, init_subscriber},
    tera::init_tera,
};

static TRACING: Lazy<()> = Lazy::new(|| {
    let subscriber = get_subscriber("test".into(), "debug".into());
    init_subscriber(subscriber);
});

static TEMPLATES: Lazy<Tera> = Lazy::new(|| init_tera());

pub struct ConfirmationLinks {
    pub html: Url,
    pub plain_text: Url,
}

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_pool: PgPool,
    pub email_server: MockServer,
    pub test_user: TestUser,
    pub api_client: reqwest::Client,
    pub tera: Tera,
    pub email_client: EmailClient,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        // Parse the body as JSON
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();
        // Extract the link from one of the request fields
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);

            let raw_link = links[0].as_str().to_owned();

            let mut confirmation_link = Url::parse(&raw_link).unwrap();
            // Let's make sure we don't call random APIs on the web
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(&body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(&body["TextBody"].as_str().unwrap());

        ConfirmationLinks { html, plain_text }
    }

    pub async fn post_publish_newsletter<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/admin/newsletters", &self.address))
            .form(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_publish_newsletter_html(&self) -> String {
        self.api_client
            .get(&format!("{}/admin/newsletters", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
            .text()
            .await
            .unwrap()
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(&format!("{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
            .text()
            .await
            .unwrap()
    }

    pub async fn get_admin_dashboard(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn get_change_password(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_change_password_html(&self) -> String {
        self.get_change_password().await.text().await.unwrap()
    }

    pub async fn post_change_password<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/admin/password", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/admin/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn dispatch_all_pending_emails(&self) {
        let mut rng = StdRng::from_seed(OsRng.gen());
        loop {
            if let ExecutionOutcome::EmptyQueue =
                try_execute_task(&self.db_pool, &self.email_client, &mut rng)
                    .await
                    .unwrap()
            {
                break;
            }
        }
    }
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    async fn store(&self, db_pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

        sqlx::query!(
            "insert into users (user_id, username, password_hash) values ($1, $2, $3)",
            self.user_id,
            self.username,
            password_hash,
        )
        .execute(db_pool)
        .await
        .expect("Failed to create test users.");
    }

    pub async fn login(&self, app: &TestApp) {
        let payload = &&serde_json::json!({
            "username": self.username,
            "password": self.password
        });
        let response = app.post_login(payload).await;
        assert_is_redirect_to(&response, "/admin/dashboard");
    }
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let tera = Lazy::force(&TEMPLATES);

    let email_server = MockServer::start().await;

    // Randomize configuration to ensure test isolation
    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        // Use a different database for each test case
        c.database.database_name = uuid::Uuid::new_v4().to_string();
        // Use a random OS port
        c.application.port = 0;
        // Use the mock server as email API
        c.email_client.base_url = email_server.uri();
        c
    };

    // Create and migrate the database
    configure_database(&configuration.database).await;

    // Launch the application as a background task
    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application");
    let port = application.port();
    let address = format!("http://127.0.0.1:{}", application.port());
    let _ = tokio::spawn(application.run_until_stopped());

    let api_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    // We return the application address to the caller!
    let test_app = TestApp {
        address,
        port,
        db_pool: get_db_pool(&configuration.database),
        email_server,
        test_user: TestUser::generate(),
        api_client,
        tera: tera.to_owned(),
        email_client: configuration.email_client.client(),
    };

    test_app.test_user.store(&test_app.db_pool).await;

    test_app
}

async fn configure_database(settings: &DatabaseSettings) -> PgPool {
    // create database
    let mut connection = PgConnection::connect_with(&settings.without_db())
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!("create database \"{}\"", &settings.database_name).as_str())
        .await
        .expect("Failed to create database");

    // migrate database
    let db_pool = PgPool::connect_with(settings.with_db())
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to migrate the database");

    db_pool
}

pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("location").unwrap(), location);
}
