use tokio;
// use rand::prelude::*;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::{SocketAddr, TcpListener};
use uuid::Uuid;
use zero2prod::config::{get_config, DatabaseSettings};

// -----------------------------------------------------------------------------

//beautiful test coz
// 1)fully decoupled from lib we're using (only spawn_app depends on it)
// 2)tests the entire api call (bb), not just the inner function (wb)
#[actix_rt::test] //replaces 1)actix_web, 2)test directives
async fn health_check_works() {
    //instantiate our server, concurrently
    let test_app = spawn_app().await;

    //instantiate a client
    let client = reqwest::Client::new();

    //execute an api call
    let response = client
        .get(format!("http://{}/health_check", test_app.address))
        .send()
        .await
        .expect("failed to call api");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

struct TestApp {
    address: String,
    db_pool: PgPool,
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    //create one-off database for testing
    //first we connect without specifying db name and we simply create a new database
    let mut connection = PgConnection::connect(&config.connection_string_without_db())
        .await
        .expect("failed to connect to db");
    connection
        .execute(&*format!(r#"CREATE DATABASE "{}";"#, config.database_name))
        .await
        .expect("failed to create db");

    //then we migrate it using the macro
    let mut connection_pool = PgPool::connect(&config.connection_string())
        .await
        .expect("failed to connect to db");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("failed to migrate db");

    //note that we're not cleaning up anywhere - it's easier to restart our postgres instance than to delete all the dbs

    connection_pool
}

async fn spawn_app() -> TestApp {
    // create psql connection
    let mut config = get_config().expect("failed to load config");
    config.database.database_name = Uuid::new_v4().to_string();
    let connection_pool = configure_database(&config.database).await;

    // generate full address
    //0 port let's the OS assign a random free port
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to assign a port");
    //get the address that was assigned
    let port = listener.local_addr().unwrap().port();
    let addr = format! {"127.0.0.1:{}", port};
    println!("randomly generated addr is {}", addr);

    // create the server
    let server =
        zero2prod::startup::run(listener, connection_pool.clone()).expect("failed to find address");
    let _ = tokio::spawn(server);

    TestApp {
        address: addr,
        db_pool: connection_pool,
    }
}

//using rand
// fn gen_addr() -> String {
//     let mut rng = thread_rng();
//     let random_port = rng.gen_range(8000..9000);
//     let addr = format!("127.0.0.1:{}", random_port);
//     println!("random port is {}", random_port);
//     addr
// }

// -----------------------------------------------------------------------------

#[actix_rt::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    // spawn app
    let test_app = spawn_app().await;

    // test route
    let client = reqwest::Client::new();
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    let response = client
        .post(&format!("http://{}/subscriptions", test_app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, response.status().as_u16());

    //test psql
    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&test_app.db_pool)
        .await
        .expect("failed to fetch subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

// "table-driven" = "parametrised" test
#[actix_rt::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    // Arrange
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];
    for (invalid_body, error_message) in test_cases {
        // Act
        let response = client
            .post(&format!("http://{}/subscriptions", test_app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");
        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            // Additional customised error message on test failure
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}
