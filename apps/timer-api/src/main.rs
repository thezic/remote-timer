use actix_web::{
    middleware,
    web::{self, ServiceConfig},
    App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
};
use timer_api::{handler, server};
use tokio::task::spawn_local;
use tracing::info;
use uuid::Uuid;

async fn index() -> impl Responder {
    actix_web::HttpResponse::Ok().body("Hello world!")
}

async fn ws_handshake(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<(Uuid,)>,
    server_handle: web::Data<server::ServerHandle>,
) -> Result<HttpResponse, Error> {
    let (res, session, stream) = actix_ws::handle(&req, stream)?;

    let (timer_id,) = path.into_inner();
    spawn_local(handler::handler(
        session,
        stream,
        (**server_handle).clone(),
        timer_id,
    ));

    Ok(res)
}

#[shuttle_runtime::main]
async fn main(
) -> shuttle_actix_web::ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
    // tracing_subscriber::fmt::init();

    // info!("Starting server at http://127.0.0.1:8080");

    let (timer_server, handle) = server::TimerServer::new();

    tokio::spawn(timer_server.run());

    let config = move |cfg: &mut ServiceConfig| {
        cfg.app_data(web::Data::new(handle.clone()))
            .service(web::resource("/").to(index))
            .service(web::resource("/ws/{id}").to(ws_handshake));
        // .wrap(middleware::Logger::default());
    };

    Ok(config.into())

    // HttpServer::new(move || {
    //     App::new()
    //         .app_data(web::Data::new(handle.clone()))
    //         .service(web::resource("/").to(index))
    //         .service(web::resource("/ws/{id}").to(ws_handshake))
    //         .wrap(middleware::Logger::default())
    // })
    // .bind(("127.0.0.1", 8080))?
    // .run()
    // .await
}
