use actix::prelude::Message;
use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, StreamHandler};
use actix_web::web::Data;
use actix_web::{get, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use mediasoup::data_producer::{DataProducerId, DataProducerOptions};
use mediasoup::prelude::{
    DataConsumer, DataConsumerId, DataConsumerOptions, DataProducer, DtlsParameters, IceCandidate,
    IceParameters, MimeTypeAudio, Router, RouterOptions, RtcpFeedback, RtpCapabilitiesFinalized,
    RtpCodecCapability, RtpCodecParametersParameters, SctpStreamParameters, Transport, TransportId,
    TransportListenIp, TransportListenIps, WebRtcTransport, WebRtcTransportOptions,
    WebRtcTransportRemoteParameters, WorkerManager, WorkerSettings,
};
use mediasoup::sctp_parameters::SctpParameters;
use mediasoup::transport::TransportGeneric;
use mediasoup::worker::{WorkerLogLevel, WorkerLogTag};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::{NonZeroU32, NonZeroU8};
use std::sync::Arc;
use uuid::Uuid;

fn media_codecs() -> Vec<RtpCodecCapability> {
    vec![RtpCodecCapability::Audio {
        mime_type: MimeTypeAudio::Opus,
        preferred_payload_type: None,
        clock_rate: NonZeroU32::new(48000).unwrap(),
        channels: NonZeroU8::new(2).unwrap(),
        parameters: RtpCodecParametersParameters::from([("useinbandfec", 1_u32.into())]),
        rtcp_feedback: vec![RtcpFeedback::TransportCc],
    }]
}

struct Transports {
    consumer: WebRtcTransport,
    producer: WebRtcTransport,
}

struct Room {
    router: Router,
    room_id: String,
    addresses: Mutex<Vec<Addr<UseConn>>>,
    producers: Mutex<Vec<DataProducer>>,
}

impl Room {
    async fn new_with_room_id(
        worker_manager: &WorkerManager,
        room_id: String,
    ) -> Result<Self, String> {
        match Self::create_router(worker_manager).await {
            Ok(router) => Ok(Self {
                router,
                room_id,
                addresses: Mutex::default(),
                producers: Mutex::default(),
            }),
            Err(error) => Err(error),
        }
    }

    async fn new(worker_manager: &WorkerManager) -> Result<Self, String> {
        let room_id = Self::generate_room_id();
        Self::new_with_room_id(worker_manager, room_id).await
    }

    fn generate_room_id() -> String {
        Uuid::new_v4().to_string()
    }

    async fn create_router(worker_manager: &WorkerManager) -> Result<Router, String> {
        let worker = worker_manager
            .create_worker({
                let mut settings = WorkerSettings::default();
                settings.log_level = WorkerLogLevel::Debug;
                settings.log_tags = vec![
                    WorkerLogTag::Info,
                    WorkerLogTag::Ice,
                    WorkerLogTag::Dtls,
                    WorkerLogTag::Rtp,
                    WorkerLogTag::Srtp,
                    WorkerLogTag::Rtcp,
                    WorkerLogTag::Rtx,
                    WorkerLogTag::Bwe,
                    WorkerLogTag::Score,
                    WorkerLogTag::Simulcast,
                    WorkerLogTag::Svc,
                    WorkerLogTag::Sctp,
                    WorkerLogTag::Message,
                ];

                settings
            })
            .await
            .map_err(|error| format!("Failed to create worker: {}", error))?;
        let router = worker
            .create_router(RouterOptions::new(media_codecs()))
            .await
            .map_err(|error| format!("Failed to create router: {}", error))?;

        Ok(router)
    }
}

struct UseConn {
    consumers: HashMap<DataConsumerId, DataConsumer>,
    transports: Transports,
    room: Arc<Room>,
}

impl UseConn {
    async fn new(room: Arc<Room>) -> Result<Self, String> {
        let mut transport_options =
            WebRtcTransportOptions::new(TransportListenIps::new(TransportListenIp {
                // Your local IP address
                ip: "192.168.10.12".parse().unwrap(),
                announced_ip: None,
            }));
        transport_options.enable_sctp = true;

        let producer_transport = room
            .router
            .create_webrtc_transport(transport_options.clone())
            .await
            .map_err(|error| format!("Failed to create producer transport: {}", error))?;
        let consumer_transport = room
            .router
            .create_webrtc_transport(transport_options.clone())
            .await
            .map_err(|error| format!("Failed to create consumer transport: {}", error))?;

        Ok(Self {
            consumers: HashMap::new(),
            transports: Transports {
                producer: producer_transport,
                consumer: consumer_transport,
            },
            room,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TransportOptions {
    id: TransportId,
    dtls_parameters: DtlsParameters,
    ice_candidates: Vec<IceCandidate>,
    ice_parameters: IceParameters,
    sctp_parameters: SctpParameters,
}

#[derive(Serialize, Message)]
#[serde(tag = "action")]
#[rtype(result = "()")]
enum ServerMessage {
    #[serde(rename_all = "camelCase")]
    Init {
        consumer_transport_options: TransportOptions,
        producer_transport_options: TransportOptions,
        router_rtp_capabilities: RtpCapabilitiesFinalized,
    },
    #[serde(rename_all = "camelCase")]
    AlreadyProduced {
        ids: Vec<DataProducerId>,
    },
    ConnectedProducerTransport,
    #[serde(rename_all = "camelCase")]
    Produced {
        id: DataProducerId,
    },
    #[serde(rename_all = "camelCase")]
    SelfProduced {
        id: DataProducerId,
    },
    ConnectedConsumerTransport,
    #[serde(rename_all = "camelCase")]
    Consumed {
        id: DataConsumerId,
        data_producer_id: DataProducerId,
        sctp_stream_parameters: SctpStreamParameters,
        label: String,
        protocol: String,
    },
}

#[derive(Deserialize, Message)]
#[serde(tag = "action")]
#[rtype(result = "()")]
enum ClientMessage {
    #[serde(rename_all = "camelCase")]
    Init {},
    #[serde(rename_all = "camelCase")]
    ConnectProducerTransport { dtls_parameters: DtlsParameters },
    #[serde(rename_all = "camelCase")]
    Produce {
        sctp_stream_parameters: SctpStreamParameters,
    },
    #[serde(rename_all = "camelCase")]
    ConnectConsumerTransport { dtls_parameters: DtlsParameters },
    #[serde(rename_all = "camelCase")]
    Consume { data_producer_id: DataProducerId },
}

#[derive(Message)]
#[rtype(result = "()")]
enum InternalMessage {
    SaveProducer(DataProducer),
    SaveConsumer(DataConsumer),
    Stop,
}

impl Actor for UseConn {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("WebSocket connection created");

        match self.transports.consumer.sctp_parameters() {
            Some(consumer_sctp) => match self.transports.producer.sctp_parameters() {
                Some(producer_sctp) => {
                    let server_init_message = ServerMessage::Init {
                        consumer_transport_options: TransportOptions {
                            id: self.transports.consumer.id(),
                            dtls_parameters: self.transports.consumer.dtls_parameters(),
                            ice_candidates: self.transports.consumer.ice_candidates().clone(),
                            ice_parameters: self.transports.consumer.ice_parameters().clone(),
                            sctp_parameters: consumer_sctp.clone(),
                        },
                        producer_transport_options: TransportOptions {
                            id: self.transports.producer.id(),
                            dtls_parameters: self.transports.producer.dtls_parameters(),
                            ice_candidates: self.transports.producer.ice_candidates().clone(),
                            ice_parameters: self.transports.producer.ice_parameters().clone(),
                            sctp_parameters: producer_sctp.clone(),
                        },
                        router_rtp_capabilities: self.room.router.rtp_capabilities().clone(),
                    };

                    println!("Router is: {}", self.room.router.id());

                    // https://levelup.gitconnected.com/websockets-in-actix-web-full-tutorial-websockets-actors-f7f9484f5086
                    let address = ctx.address();
                    address.do_send(server_init_message);
                    self.room.addresses.lock().push(address);
                }
                None => {
                    println!("sctp parameters for producer does not exist");
                    ctx.close(Some(ws::CloseReason::from(ws::CloseCode::Error)));
                }
            },
            None => {
                println!("sctp parameters for consumer does not exist");
                ctx.close(Some(ws::CloseReason::from(ws::CloseCode::Error)));
            }
        }
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        println!("WebSocket connection closed");
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for UseConn {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                println!("receive pong {:?}", msg);
                ctx.pong(&msg)
            }
            Ok(ws::Message::Pong(_)) => {}
            Ok(ws::Message::Text(text)) => match serde_json::from_str::<ClientMessage>(&text) {
                Ok(message) => {
                    ctx.address().do_send(message);
                }
                Err(error) => {
                    eprintln!("Failed to parse client message: {}\n{}", error, text);
                }
            },
            Ok(ws::Message::Binary(bin)) => {
                println!("receive binary {:?}", bin);
                ctx.binary(bin)
            }
            Ok(ws::Message::Close(reason)) => ctx.close(reason),
            _ => (),
        }
    }
}

impl Handler<ClientMessage> for UseConn {
    type Result = ();

    fn handle(&mut self, message: ClientMessage, ctx: &mut Self::Context) {
        match message {
            ClientMessage::Init {} => {
                let address = ctx.address();
                address.do_send(ServerMessage::AlreadyProduced {
                    ids: self.room.producers.lock().iter().map(|p| p.id()).collect(),
                })
            }
            ClientMessage::ConnectProducerTransport { dtls_parameters } => {
                let address = ctx.address();
                let transport = self.transports.producer.clone();
                actix::spawn(async move {
                    match transport
                        .connect(WebRtcTransportRemoteParameters { dtls_parameters })
                        .await
                    {
                        Ok(_) => {
                            address.do_send(ServerMessage::ConnectedProducerTransport);
                            println!("Producer transport connected");
                        }
                        Err(error) => {
                            eprintln!("Failed to connect producer transport: {}", error);
                            address.do_send(InternalMessage::Stop);
                        }
                    }
                });
            }
            ClientMessage::Produce {
                sctp_stream_parameters,
            } => {
                let address = ctx.address();

                let transport = self.transports.producer.clone();
                let addresses = self.room.addresses.lock().clone();

                actix::spawn(async move {
                    match transport
                        .produce_data(DataProducerOptions::new_sctp(sctp_stream_parameters))
                        .await
                    {
                        Ok(producer) => {
                            let id = producer.id();

                            address.do_send(InternalMessage::SaveProducer(producer));
                            address.do_send(ServerMessage::SelfProduced { id });
                            // Need to broadcast for all users in the same route
                            // https://github.com/actix/actix-web/issues/704#issuecomment-466115447
                            for addr in addresses.iter() {
                                addr.do_send(ServerMessage::Produced { id });
                            }

                            println!("producer created: {}", id);
                        }
                        Err(error) => {
                            eprintln!("Failed to create producer: {}", error);
                            address.do_send(InternalMessage::Stop);
                        }
                    }
                });
            }
            ClientMessage::ConnectConsumerTransport { dtls_parameters } => {
                let address = ctx.address();
                let transport = self.transports.consumer.clone();

                actix::spawn(async move {
                    match transport
                        .connect(WebRtcTransportRemoteParameters { dtls_parameters })
                        .await
                    {
                        Ok(_) => {
                            address.do_send(ServerMessage::ConnectedConsumerTransport);
                            println!("Consumer transport connected");
                        }
                        Err(error) => {
                            eprintln!("Failed to connect consumer transport: {}", error);
                            address.do_send(InternalMessage::Stop);
                        }
                    }
                });
            }
            ClientMessage::Consume { data_producer_id } => {
                let address = ctx.address();
                let transport = self.transports.consumer.clone();

                actix::spawn(async move {
                    let options = DataConsumerOptions::new_direct(data_producer_id);

                    match transport.consume_data(options).await {
                        Ok(consumer) => {
                            let id = consumer.id();
                            let label = consumer.label().clone();
                            let protocol = consumer.protocol().clone();
                            match consumer.sctp_stream_parameters() {
                                Some(sctp_stream_parameters) => {
                                    address.do_send(ServerMessage::Consumed {
                                        id,
                                        data_producer_id,
                                        sctp_stream_parameters,
                                        label,
                                        protocol,
                                    });
                                    address.do_send(InternalMessage::SaveConsumer(consumer));
                                    println!("consumer created: {}", id);
                                }
                                None => {
                                    eprintln!(
                                        "sctp stream parameters does not exist in consumer: {}",
                                        id
                                    );
                                }
                            }
                        }
                        Err(error) => {
                            eprintln!("Failed to create consumer: {}", error);
                            address.do_send(InternalMessage::Stop);
                        }
                    }
                });
            }
        }
    }
}
// TODO: Remove producer form room.producers when producer/consumer is closed

impl Handler<ServerMessage> for UseConn {
    type Result = ();

    fn handle(&mut self, message: ServerMessage, ctx: &mut Self::Context) {
        ctx.text(serde_json::to_string(&message).unwrap());
    }
}

impl Handler<InternalMessage> for UseConn {
    type Result = ();

    fn handle(&mut self, message: InternalMessage, ctx: &mut Self::Context) {
        match message {
            InternalMessage::Stop => {
                ctx.stop();
            }
            InternalMessage::SaveProducer(producer) => {
                self.room.producers.lock().push(producer);
            }
            InternalMessage::SaveConsumer(consumer) => {
                self.consumers.insert(consumer.id(), consumer);
            }
        }
    }
}

async fn index(
    req: HttpRequest,
    room: Data<Room>,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    let r = Arc::clone(&room);
    match UseConn::new(r).await {
        Ok(server) => ws::start(server, &req, stream),
        Err(error) => {
            eprintln!("{}", error);

            Ok(HttpResponse::InternalServerError().finish())
        }
    }
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let worker_manager = WorkerManager::new();
    match Room::new(&worker_manager).await {
        Ok(room) => {
            println!("room is creatd: {}", room.room_id);
            let data = Data::new(room);
            HttpServer::new(move || {
                App::new()
                    .service(hello)
                    .app_data(data.clone())
                    .route("/ws", web::get().to(index))
            })
            .workers(2)
            .bind("127.0.0.1:3000")?
            .run()
            .await
        }
        Err(error) => {
            let err = std::io::Error::new(std::io::ErrorKind::Other, error);
            Err(err)
        }
    }
}
