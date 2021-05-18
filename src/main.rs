use std::net::TcpListener;
use zero2prod::config::get_config;
use zero2prod::startup::run;

#[actix_web::main] //needed to have an async runtime, because rust by default doesn't provide one
pub async fn main() -> std::io::Result<()> {
    //panic if no config
    let _config = get_config().expect("failed to load config");

    //? = bubble up the io error
    let listener = TcpListener::bind("localhost:8000").expect("failed to bind");
    run(listener)?.await
}
