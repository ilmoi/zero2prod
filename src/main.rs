use actix_web::{web, App, HttpRequest, HttpServer, Responder};

async fn greet(req: HttpRequest) -> impl Responder {
    //not the only accepted signature
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}\n", name)
}

#[actix_web::main] //needed to have an async runtime, because rust by default doesn't provide one
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        //httpserver handles all the transport level concerns
        //note how it takes a closure = an app factory
        App::new() //app handles app logic - routing, middleware, request handlers
            // iterate over all registered endpoints until find a match (both route and guard)
            .route("/", web::get().to(greet)) // then pass over to handler - in this case greet()
            .route("/{name}", web::get().to(greet))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await //we chain this onto the fn, which effectively executes the future
}
