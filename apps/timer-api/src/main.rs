use actix_web::{
    middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
};
use timer_api::{config::HandlerConfig, handler, server};
use tokio::task::spawn_local;
use uuid::Uuid;

async fn index() -> impl Responder {
    actix_web::HttpResponse::Ok().body("Hello world!")
}

async fn health() -> impl Responder {
    actix_web::HttpResponse::Ok().body("OK")
}

async fn ws_handshake(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<(Uuid,)>,
    server_handle: web::Data<server::ServerHandle>,
    handler_config: web::Data<HandlerConfig>,
) -> Result<HttpResponse, Error> {
    let (res, session, stream) = actix_ws::handle(&req, stream)?;

    let (timer_id,) = path.into_inner();
    spawn_local(handler::handler(
        session,
        stream,
        (**server_handle).clone(),
        timer_id,
        (**handler_config).clone(),
    ));

    Ok(res)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let (timer_server, handle) = server::TimerServer::new();

    tokio::spawn(timer_server.run());

    // Get port from environment variable or default to 8080
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    let bind_address = format!("0.0.0.0:{}", port);

    tracing::info!("Starting server on {}", bind_address);

    let handler_config = HandlerConfig::default();

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(handle.clone()))
            .app_data(web::Data::new(handler_config.clone()))
            .service(web::resource("/").to(index))
            .service(web::resource("/health").to(health))
            .service(web::resource("/ws/{id}").to(ws_handshake))
    })
    .bind(&bind_address)?
    .run()
    .await
}
