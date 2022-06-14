use actix::{prelude::Message, Actor, ActorContext, AsyncContext, Handler, StreamHandler};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use mediasoup::{
    data_producer::{DataProducer, DataProducerId, DataProducerOptions},
    prelude::{
        DataConsumer, DataConsumerId, DataConsumerOptions, DtlsParameters, IceCandidate,
        IceParameters, SctpStreamParameters, TransportListenIp, TransportListenIps,
        WebRtcTransport, WebRtcTransportOptions, WebRtcTransportRemoteParameters, WorkerSettings,
    },
    router::{Router, RouterOptions},
    rtp_parameters::{
        MimeTypeAudio, RtcpFeedback, RtpCapabilitiesFinalized, RtpCodecCapability,
        RtpCodecParametersParameters,
    },
    sctp_parameters::SctpParameters,
    transport::{Transport, TransportId},
    worker::{WorkerLogLevel, WorkerLogTag},
    worker_manager::WorkerManager,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::{NonZeroU32, NonZeroU8};

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

struct DataConn {
    consumers: HashMap<DataConsumerId, DataConsumer>,
    producers: Vec<DataProducer>,
    router: Router,
    transports: Transports,
}

impl DataConn {
    async fn new(worker_manager: &WorkerManager) -> Result<Self, String> {
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

        // This example uses only 2 transports.
        let mut transport_options =
            WebRtcTransportOptions::new(TransportListenIps::new(TransportListenIp {
                // Your local IP address
                ip: "192.168.10.12".parse().unwrap(),
                announced_ip: None,
            }));
        transport_options.enable_sctp = true;
        let producer_transport = router
            .create_webrtc_transport(transport_options.clone())
            .await
            .map_err(|error| format!("Failed to create producer transport: {}", error))?;
        let consumer_transport = router
            .create_webrtc_transport(transport_options.clone())
            .await
            .map_err(|error| format!("Failed to create consumer transport: {}", error))?;

        Ok(Self {
            consumers: HashMap::new(),
            producers: vec![],
            transports: Transports {
                producer: producer_transport,
                consumer: consumer_transport,
            },
            router,
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

    ConnectedProducerTransport,
    #[serde(rename_all = "camelCase")]
    Produced {
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

impl Actor for DataConn {
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
                        router_rtp_capabilities: self.router.rtp_capabilities().clone(),
                    };

                    ctx.address().do_send(server_init_message);
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

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for DataConn {
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
            Ok(ws::Message::Close(reason)) => {
                println!("receive close message: {:?}", reason);
                ctx.close(reason);
            }
            _ => (),
        }
    }
}

impl Handler<ClientMessage> for DataConn {
    type Result = ();

    fn handle(&mut self, message: ClientMessage, ctx: &mut Self::Context) {
        match message {
            ClientMessage::Init {} => {
                println!("Client initialized");
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

                actix::spawn(async move {
                    match transport
                        .produce_data(DataProducerOptions::new_sctp(sctp_stream_parameters))
                        .await
                    {
                        Ok(producer) => {
                            let id = producer.id();
                            address.do_send(ServerMessage::Produced { id });
                            address.do_send(InternalMessage::SaveProducer(producer));
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

impl Handler<ServerMessage> for DataConn {
    type Result = ();

    fn handle(&mut self, message: ServerMessage, ctx: &mut Self::Context) {
        ctx.text(serde_json::to_string(&message).unwrap());
    }
}

impl Handler<InternalMessage> for DataConn {
    type Result = ();

    fn handle(&mut self, message: InternalMessage, ctx: &mut Self::Context) {
        match message {
            InternalMessage::Stop => {
                println!("someone call stop message");
                ctx.stop();
            }
            InternalMessage::SaveProducer(producer) => {
                self.producers.push(producer);
            }
            InternalMessage::SaveConsumer(consumer) => {
                self.consumers.insert(consumer.id(), consumer);
            }
        }
    }
}

async fn index(
    req: HttpRequest,
    worker_manager: web::Data<WorkerManager>,
    stream: actix_web::web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    match DataConn::new(&worker_manager).await {
        Ok(server) => ws::start(server, &req, stream),
        Err(error) => {
            eprintln!("{}", error);
            Ok(HttpResponse::InternalServerError().finish())
        }
    }
}
#[actix_web::get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let worker_manager = web::Data::new(WorkerManager::new());
    HttpServer::new(move || {
        App::new()
            .service(hello)
            .app_data(worker_manager.clone())
            .route("/ws", web::get().to(index))
    })
    .workers(2)
    .bind("127.0.0.1:3000")?
    .run()
    .await
}
