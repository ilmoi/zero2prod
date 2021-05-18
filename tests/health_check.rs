use tokio;
// use rand::prelude::*;
use sqlx::{Connection, PgConnection};
use std::net::{SocketAddr, TcpListener};
use zero2prod::config::get_config;

// -----------------------------------------------------------------------------

//beautiful test coz
// 1)fully decoupled from lib we're using (only spawn_app depends on it)
// 2)tests the entire api call (bb), not just the inner function (wb)
#[actix_rt::test] //replaces 1)actix_web, 2)test directives
async fn health_check_works() {
    //instantiate our server, concurrently
    let addr = spawn_app();

    //instantiate a client
    let client = reqwest::Client::new();

    //execute an api call
    let response = client
        .get(format!("http://{}/health_check", addr))
        .send()
        .await
        .expect("failed to call api");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

fn spawn_app() -> SocketAddr {
    //generate full address
    let addr = gen_addr();
    let listener = TcpListener::bind(addr).expect("failed to bind");
    let server = zero2prod::startup::run(listener).expect("failed to find address");
    let _ = tokio::spawn(server);
    addr
}

//using rand
// fn gen_addr() -> String {
//     let mut rng = thread_rng();
//     let random_port = rng.gen_range(8000..9000);
//     let addr = format!("127.0.0.1:{}", random_port);
//     println!("random port is {}", random_port);
//     addr
// }

fn gen_addr() -> SocketAddr {
    //0 port let's the OS assign a random free port
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to assign a port");
    //get the address that was assigned
    let addr = listener.local_addr().unwrap();
    println!("randomly generated addr is {}", addr);
    addr
}

// -----------------------------------------------------------------------------

#[actix_rt::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    // spawn app
    let app_address = spawn_app();

    // test route
    let client = reqwest::Client::new();
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    let response = client
        .post(&format!("http://{}/subscriptions", &app_address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, response.status().as_u16());

    // -----------------------------------------------------------------------------

    // create psql connection
    let config = get_config().expect("failed to load config");
    let conn_str = format!(
        "postgres://{}:{}@{}:{}/{}",
        config.database.username,
        config.database.password,
        config.database.host,
        config.database.port,
        config.database.database_name
    );
    let mut connection = PgConnection::connect(&conn_str)
        .await
        .expect("failed to connect to db");

    //test psql
    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&mut connection)
        .await
        .expect("failed to fetch subscription");

    assert_eq!(saved.email, "urusula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

// "table-driven" = "parametrised" test
#[actix_rt::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    // Arrange
    let app_address = spawn_app();
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];
    for (invalid_body, error_message) in test_cases {
        // Act
        let response = client
            .post(&format!("http://{}/subscriptions", &app_address))
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
