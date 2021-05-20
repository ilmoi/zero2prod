use zero2prod::config::get_config;
use zero2prod::startup::Application;
use zero2prod::telem::{get_subscriber, init_subscriber};

#[actix_web::main] //needed to have an async runtime, because rust by default doesn't provide one
pub async fn main() -> std::io::Result<()> {
    //LOGGING
    //this is us calling set_logger so that our app knows what to do with logs
    //specifically we opted for env_logger, which is used for when you want to print to terminal
    //specifically we'll be printing all logs from "info" and above
    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    //TRACING
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let config = get_config().expect("failed to load config");
    let app = Application::build(config)
        .await
        .expect("failed to start app");
    app.run_until_stopped().await?;
    Ok(())
}
