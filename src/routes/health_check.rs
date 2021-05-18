use actix_web::HttpResponse;

// async fn greet(req: HttpRequest) -> impl Responder {
//     //not the only accepted signature
//     let name = req.match_info().get("name").unwrap_or("World");
//     format!("Hello {}\n", name)
// }


pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
