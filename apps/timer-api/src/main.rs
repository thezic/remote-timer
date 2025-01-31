use actix_web::{
    web::{self, ServiceConfig},
    Error, HttpRequest, HttpResponse, Responder,
};
use timer_api::{handler, server};
use tokio::task::spawn_local;
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
    let (timer_server, handle) = server::TimerServer::new();

    tokio::spawn(timer_server.run());

    let config = move |cfg: &mut ServiceConfig| {
        cfg.app_data(web::Data::new(handle.clone()))
            .service(web::resource("/").to(index))
            .service(web::resource("/ws/{id}").to(ws_handshake));
    };

    Ok(config.into())
}
