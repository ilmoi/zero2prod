use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::config::get_config;
use zero2prod::startup::run;

#[actix_web::main] //needed to have an async runtime, because rust by default doesn't provide one
pub async fn main() -> std::io::Result<()> {
    // create psql connection
    let config = get_config().expect("failed to load config");
    let conn_str = config.database.connection_string();

    //use PgPool instead of PgConnection.
    // using single connection + Arc = doesn't allow mutable connections, which we need for sqlx's .execute to work.
    // using single connection + Mutex = only allows one in total at a time = slow.
    // PgPool bypasses both creating a pool of mutable connections.
    let connection_pool = PgPool::connect(&conn_str)
        .await
        .expect("failed to connect to db");

    // setup address + listener
    let address = format!("localhost:{}", config.application_port);
    let listener = TcpListener::bind(address).expect("failed to bind");

    run(listener, connection_pool)?.await
}
