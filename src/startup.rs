use std::net::TcpListener;

use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};

use crate::routes::health_check;
use crate::routes::subscribe;
use sqlx::PgPool;

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

// now using a listener
pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    //we need connection to be cloneable to spin up multiple copies of App, one for each core on our machine
    // let connection = Arc::new(connection);

    //wrap the pool using web::Data, which under the hood is an Arc smart pointer
    let db_pool = web::Data::new(db_pool);

    let server = HttpServer::new(move || {
        App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            //add state to our application - note .data wraps inside of Arc::new()
            // .data(connection.clone())
            //with pg_pool instead of connection we're using app_data, coz we don't want Arc::new(Arc::new()) - app_data doesn't perform an additional layer of wrapping
            .app_data(db_pool.clone())
    })
    .listen(listener)?
    .run();
    //no more await!
    Ok(server)
}
