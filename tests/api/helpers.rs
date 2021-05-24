use once_cell::sync::Lazy;
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, PgConnection, PgPool};

use reqwest::Url;
use wiremock::MockServer;
use zero2prod::config::{get_config, DatabaseSettings};
use zero2prod::startup::{get_connection_pool, Application};
use zero2prod::telem::{get_subscriber, init_subscriber};

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub email_server: MockServer,
    pub port: u16,
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    //create one-off database for testing
    //first we connect without specifying db name and we simply create a new database
    // let mut connection = PgConnection::connect(&config.connection_string_without_db())
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("failed to connect to db");
    connection
        .execute(&*format!(r#"CREATE DATABASE "{}";"#, config.database_name))
        .await
        .expect("failed to create db");

    //then we migrate it using the macro
    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("failed to connect to db");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("failed to migrate db");

    //note that we're not cleaning up anywhere - it's easier to restart our postgres instance than to delete all the dbs

    connection_pool
}

//this is so that our subscriber is only initialized once
//this also hides the test logs unless we enable them back on with TEST_LOG=true
static TRACING: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber("test".into(), "debug".into(), std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber("test".into(), "debug".into(), std::io::sink);
        init_subscriber(subscriber);
    }
});

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING); // init the subscriber ONCE

    let email_server = MockServer::start().await;

    //build the config
    let config = {
        let mut c = get_config().expect("failed to load config"); //fetch the config
        c.database.database_name = Uuid::new_v4().to_string(); //invest a new db name
        c.app.port = 0; //change port to 0 for testing (will reassign to random sys port)
        c.email_client.base_url = email_server.uri();
        c
    };

    //create the newly named db
    configure_database(&config.database).await;

    //launch the app
    let app = Application::build(config.clone())
        .await
        .expect("failed to build");
    let port = app.port();
    let _ = tokio::spawn(app.run_until_stopped());

    //return an instance
    TestApp {
        address: format!("localhost:{}", port),
        db_pool: get_connection_pool(&config.database)
            .await
            .expect("failed to get connection pool"),
        email_server,
        port,
    }
}

// -----------------------------------------------------------------------------
// client to interact with our own api

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("http://{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

// -----------------------------------------------------------------------------
// confirmation links

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

impl TestApp {
    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        //deserialize into a valid json -https://docs.serde.rs/serde_json/enum.Value.html
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        // Extract the link from one of the request fields.
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            links[0].as_str().to_owned()
        };

        let html_raw_link = &get_link(&body["HtmlBody"].as_str().unwrap());
        let text_raw_link = &get_link(&body["HtmlBody"].as_str().unwrap());
        let html = Url::parse(html_raw_link).unwrap();
        let plain_text = Url::parse(text_raw_link).unwrap();

        ConfirmationLinks { html, plain_text }
    }
}
