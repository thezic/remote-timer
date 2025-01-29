use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use timer_api::{handler, server};
use tokio::task::spawn_local;
use tracing::info;

async fn index() -> impl Responder {
    actix_web::HttpResponse::Ok().body("Hello world!")
}

async fn ws_handshake(
    req: HttpRequest,
    stream: web::Payload,
    server_handle: web::Data<server::ServerHandle>,
) -> Result<HttpResponse, Error> {
    let (res, session, stream) = actix_ws::handle(&req, stream)?;

    spawn_local(handler::handler(session, stream, (**server_handle).clone()));

    Ok(res)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting server at http://127.0.0.1:8080");

    let (timer_server, handle) = server::TimerServer::new();

    tokio::spawn(timer_server.run());

    HttpServer::new(move || {
        App::new()
            .app_data(handle.clone())
            .service(web::resource("/").to(index))
            .service(web::resource("/ws").to(ws_handshake))
            .wrap(middleware::Logger::default())
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
