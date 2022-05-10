use actix_web::{get, web, Responder, HttpResponse, HttpServer, App, HttpRequest, Error};
use actix::{Actor, StreamHandler};
use actix_web_actors::ws;

struct ExampleWS;

impl Actor for ExampleWS {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ExampleWS {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                println!("receive pong {:?}", msg);
                ctx.pong(&msg)
            },
            Ok(ws::Message::Text(text)) => {
                println!("receive text {:?}", text);
                ctx.text(text)
            },
            Ok(ws::Message::Binary(bin)) => {
                println!("receive binary {:?}", bin);
                ctx.binary(bin)
            },
            _ => (),
        }
    }
}

async fn index(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let resp = ws::start(ExampleWS, &req, stream);
    println!("{:?}", resp);
    resp
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(hello)
            .route("/ws", web::get().to(index))
    })
        .bind("127.0.0.1:3000")?
        .run()
        .await
}
