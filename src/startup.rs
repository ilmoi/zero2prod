use std::net::TcpListener;

use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;
use tracing_actix_web::TracingLogger;

use crate::config::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes::health_check;
use crate::routes::rename;
use crate::routes::subscribe;
use sqlx::postgres::PgPoolOptions;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(config: Settings) -> Result<Self, std::io::Error> {
        //PSQL
        let connection_pool = get_connection_pool(&config.database)
            .await
            .expect("failed to connect to db");

        //EMAIL
        let sender_email = config
            .email_client
            .sender()
            .expect("failed to get sender email");
        let email_client = EmailClient::new(
            config.email_client.base_url,
            sender_email,
            config.email_client.auth_token,
        );

        //RUN
        let address = format!("{}:{}", config.app.host, config.app.port);
        println!("Address is: {}", address);
        let listener = TcpListener::bind(&address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(listener, connection_pool, email_client)?;

        Ok(Self { port, server })
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    //an expressively named fn to let other parts of the code know this fn only returns when app is stopped
    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub async fn get_connection_pool(config: &DatabaseSettings) -> Result<PgPool, sqlx::Error> {
    // use pg options instead of string, so that we can enable TLS on db connection
    // let conn_str = config.database.connection_string();

    //use PgPool instead of PgConnection.
    // using single connection + Arc = doesn't allow mutable connections, which we need for sqlx's .execute to work.
    // using single connection + Mutex = only allows one in total at a time = slow.
    // PgPool bypasses both creating a pool of mutable connections.
    PgPoolOptions::new()
        .connect_timeout(std::time::Duration::from_secs(2))
        .connect_with(config.with_db())
        .await
}

// now using a listener
pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
) -> Result<Server, std::io::Error> {
    //we need connection to be cloneable to spin up multiple copies of App, one for each core on our machine
    // let connection = Arc::new(connection);

    //wrap the pool using web::Data, which under the hood is an Arc smart pointer
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);

    let server = HttpServer::new(move || {
        App::new()
            //LOGGING
            // .wrap(Logger::default())
            //TRACING
            .wrap(TracingLogger::default()) // this lets us track request_id all the way from request start to end, not only in functions we labeled in subscriptions.rs. It's designed as a drop in replacement for the above logger
            //ROUTES
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/rename", web::post().to(rename))
            //APP STATE
            //note .data wraps inside of Arc::new()
            // .data(connection.clone())
            //with pg_pool instead of connection we're using app_data, coz we don't want Arc::new(Arc::new()) - app_data doesn't perform an additional layer of wrapping
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
    })
    .listen(listener)?
    .run();
    //no more await!
    Ok(server)
}

//return a server, blocking
// pub async fn run() -> std::io::Result<()> {
//     HttpServer::new(|| {
//         //httpserver handles all the transport level concerns
//         //note how it takes a closure = an app factory
//         App::new() //app handles app logic - routing, middleware, request handlers
//             // iterate over all registered endpoints until find a match (both 1)route and 2)guard)
//             .route("/", web::get().to(greet)) // then pass over to handler - in this case greet()
//             .route("/health_check", web::get().to(health_check))
//             .route("/{name}", web::get().to(greet))
//     })
//         .bind("127.0.0.1:8000")?
//         .run()
//         .await //we chain this onto the fn, which effectively executes the future
// }

//return a server, concurrent
//note1: dropped async/await
//note2: return a server instance on happy path
// pub fn run(address: &str) -> Result<Server, std::io::Error> {
//     let server = HttpServer::new(|| {
//         App::new()
//             .route("/health_check", web::get().to(health_check))
//     })
//         .bind(address)?
//         .run();
//     //no more await!
//     Ok(server)
// }
