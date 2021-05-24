use crate::helpers::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// -----------------------------------------------------------------------------
// testing http

#[actix_rt::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    // spawn app
    let test_app = spawn_app().await;

    //setup a mock server - w/o this we'd get a 500
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&test_app.email_server)
        .await;

    // test route
    let body = String::from("name=le%20guin&email=ursula_le_guin%40gmail.com");
    let response = test_app.post_subscriptions(body).await;
    assert_eq!(200, response.status().as_u16());
}

#[actix_rt::test]
async fn subscribe_persists_data_in_db() {
    // spawn app
    let test_app = spawn_app().await;

    //setup a mock server - w/o this we'd get a 500
    // Mock::given(path("/email"))
    //     .and(method("POST"))
    //     .respond_with(ResponseTemplate::new(200))
    //     .mount(&test_app.email_server)
    //     .await;

    // test route
    let body = String::from("name=le%20guin&email=ursula_le_guin%40gmail.com");
    test_app.post_subscriptions(body).await;

    //test psql
    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
        .fetch_one(&test_app.db_pool)
        .await
        .expect("failed to fetch subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "pending_confirmation");
}

#[actix_rt::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_invalid() {
    // spawn app
    let test_app = spawn_app().await;

    //setup a mock server - w/o this we'd get a 500
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&test_app.email_server)
        .await;

    // test route
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];
    for (body, description) in test_cases {
        // Act
        let response = test_app.post_subscriptions(String::from(body)).await;
        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 400 when the payload was {}.",
            description
        );
    }
}

// "table-driven" = "parametrised" test
#[actix_rt::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    // Arrange
    let test_app = spawn_app().await;

    //setup a mock server - w/o this we'd get a 500
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&test_app.email_server)
        .await;

    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];
    for (invalid_body, error_message) in test_cases {
        // Act
        let response = test_app
            .post_subscriptions(String::from(invalid_body))
            .await;
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

// -----------------------------------------------------------------------------
// testing POST email

#[actix_rt::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    // Arrange
    let test_app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    //setup a mock server AND check that ti's actually hit once
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1) // Assert
        .mount(&test_app.email_server)
        .await;

    // Act
    test_app.post_subscriptions(body.into()).await;

    // CHECKING THE BODY
    let email_request = &test_app.email_server.received_requests().await.unwrap()[0];
    let links = test_app.get_confirmation_links(&email_request);
    assert_eq!(links.html, links.plain_text); //another assert to check links that we extract are equal
}

// -----------------------------------------------------------------------------
//testing GET email

#[actix_rt::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    let test_app = spawn_app().await;

    let response = reqwest::Client::new()
        .get(&format!(
            "http://{}/subscriptions/confirm",
            test_app.address
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400)
}

#[actix_rt::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    // hist the post endpoint to get the link
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;
    app.post_subscriptions(body.into()).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];

    let links = app.get_confirmation_links(&email_request);
    let mut link = links.html;
    assert_eq!(link.host_str().unwrap(), "127.0.0.1");

    //set the port
    link.set_port(Some(app.port));

    // now do a get request to that link
    let response = reqwest::get(link).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

// -----------------------------------------------------------------------------
// testing sub tokens

#[actix_rt::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
    // Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;
    app.post_subscriptions(body.into()).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(&email_request);
    // Act

    println!("html link is {}", confirmation_links.html);
    println!("plain link is {}", confirmation_links.plain_text);

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    // Assert
    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");
    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "confirmed");
}

// -----------------------------------------------------------------------------
// test quality of errors

#[actix_rt::test]
async fn store_token_fails() {
    // Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    // Sabotage the database
    sqlx::query!("ALTER TABLE subscription_tokens DROP COLUMN sub_token;",)
        .execute(&app.db_pool)
        .await
        .unwrap();
    // Act
    let response = app.post_subscriptions(body.into()).await;
    // Assert
    assert_eq!(response.status().as_u16(), 500);
}

#[actix_rt::test]
async fn insert_sub_fails() {
    // Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    // Sabotage the database
    // sqlx::query!("ALTER TABLE subscriptions DROP COLUMN name;",)
    //     .execute(&app.db_pool)
    //     .await
    //     .unwrap();
    // Act
    let response = app.post_subscriptions(body.into()).await;
    let response = app.post_subscriptions(body.into()).await; //2nd time should fail
                                                              // Assert
    assert_eq!(response.status().as_u16(), 500);
}

#[actix_rt::test]
async fn send_email_fails() {
    // Arrange
    let app = spawn_app().await;

    // todo this test fails if we don't give it a mock server
    // Mock::given(path("/email"))
    //     .and(method("POST"))
    //     .respond_with(ResponseTemplate::new(200))
    //     .mount(&app.email_server)
    //     .await;

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    // Act
    let response = app.post_subscriptions(body.into()).await;
    // Assert
    assert_eq!(response.status().as_u16(), 500);
}
