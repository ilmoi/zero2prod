use std::net::TcpListener;

use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};

use crate::routes::health_check;
use crate::routes::subscribe;

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
pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
    })
    .listen(listener)?
    .run();
    //no more await!
    Ok(server)
}
