use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::config::get_config;
use zero2prod::startup::run;
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

    //PSQL
    let config = get_config().expect("failed to load config");
    let conn_str = config.database.connection_string();
    //use PgPool instead of PgConnection.
    // using single connection + Arc = doesn't allow mutable connections, which we need for sqlx's .execute to work.
    // using single connection + Mutex = only allows one in total at a time = slow.
    // PgPool bypasses both creating a pool of mutable connections.
    let connection_pool = PgPool::connect(&conn_str)
        .await
        .expect("failed to connect to db");

    //RUN
    let address = format!("localhost:{}", config.application_port);
    let listener = TcpListener::bind(address).expect("failed to bind");
    run(listener, connection_pool)?.await
}
