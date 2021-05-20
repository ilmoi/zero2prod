use crate::helpers::spawn_app;

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
