import { Device } from "mediasoup-client";
import { Producer } from "mediasoup-client/lib/Producer";
import { ConsumerOptions } from "mediasoup-client/lib/Consumer";
import {
  RtpCapabilities,
  RtpParameters,
  MediaKind,
} from "mediasoup-client/lib/RtpParameters";
import {
  DtlsParameters,
  Transport,
  TransportOptions,
} from "mediasoup-client/lib/Transport";

type Brand<K, T> = K & { __brand: T };

type ConsumerId = Brand<string, "ConsumerId">;
type ProducerId = Brand<string, "ProducerId">;

interface ServerInit {
  action: "Init";
  consumerTransportOptions: TransportOptions;
  producerTransportOptions: TransportOptions;
  routerRtpCapabilities: RtpCapabilities;
}

interface ServerConnectedProducerTransport {
  action: "ConnectedProducerTransport";
}

interface ServerProduced {
  action: "Produced";
  id: ProducerId;
}

interface ServerConnectedConsumerTransport {
  action: "ConnectedConsumerTransport";
}

interface ServerConsumed {
  action: "Consumed";
  id: ConsumerId;
  kind: MediaKind;
  rtpParameters: RtpParameters;
}

type ServerMessage =
  | ServerInit
  | ServerConnectedProducerTransport
  | ServerProduced
  | ServerConnectedConsumerTransport
  | ServerConsumed;

interface ClientInit {
  action: "Init";
  rtpCapabilities: RtpCapabilities;
}

interface ClientConnectProducerTransport {
  action: "ConnectProducerTransport";
  dtlsParameters: DtlsParameters;
}

interface ClientConnectConsumerTransport {
  action: "ConnectConsumerTransport";
  dtlsParameters: DtlsParameters;
}

interface ClientProduce {
  action: "Produce";
  kind: MediaKind;
  rtpParameters: RtpParameters;
}

interface ClientConsume {
  action: "Consume";
  producerId: ProducerId;
}

interface ClientConsumerResume {
  action: "ConsumerResume";
  id: ConsumerId;
}

type ClientMessage =
  | ClientInit
  | ClientConnectProducerTransport
  | ClientProduce
  | ClientConnectConsumerTransport
  | ClientConsume
  | ClientConsumerResume;

const sendMessage = (ws: WebSocket, message: ClientMessage) => {
  ws.send(JSON.stringify(message));
};

const init = async () => {
  const receivePreview = document.querySelector("#receive") as HTMLAudioElement;
  receivePreview.onloadedmetadata = () => {
    receivePreview.play();
  };

  const receiveMediaStream = new MediaStream();
  receiveMediaStream.onaddtrack = (ev) => {
    console.log("track added: ", ev);
  };
  const ws = new WebSocket("ws://localhost:3000/ws");

  const device = new Device();
  let producerTransport: Transport | undefined;
  let consumerTransport: Transport | undefined;

  {
    const waitingForResponse: Map<ServerMessage["action"], Function> =
      new Map();

    ws.onmessage = async (message) => {
      const decodedMessage: ServerMessage = JSON.parse(message.data);

      switch (decodedMessage.action) {
        case "Init":
          await device.load({
            routerRtpCapabilities: decodedMessage.routerRtpCapabilities,
          });

          sendMessage(ws, {
            action: "Init",
            rtpCapabilities: device.rtpCapabilities,
          });

          producerTransport = device.createSendTransport(
            decodedMessage.producerTransportOptions
          );

          producerTransport
            .on("connect", ({ dtlsParameters }, success) => {
              sendMessage(ws, {
                action: "ConnectProducerTransport",
                dtlsParameters,
              });
              waitingForResponse.set("ConnectedProducerTransport", () => {
                success();
                console.log("Producer transport connected");
              });
            })
            .on("produce", ({ kind, rtpParameters }, success) => {
              sendMessage(ws, {
                action: "Produce",
                kind,
                rtpParameters,
              });
              waitingForResponse.set("Produced", ({ id }: { id: string }) => {
                success({ id });
              });
            });

          const mediaStream = await navigator.mediaDevices.getUserMedia({
            audio: true,
          });

          const producers: Array<Producer> = [];
          for (const track of mediaStream.getTracks()) {
            const producer = await producerTransport.produce({ track });

            producers.push(producer);
            console.log(`${track.kind} producer created:`, producer);
          }

          consumerTransport = device.createRecvTransport(
            decodedMessage.consumerTransportOptions
          );

          consumerTransport.on("connect", ({ dtlsParameters }, success) => {
            sendMessage(ws, {
              action: "ConnectConsumerTransport",
              dtlsParameters,
            });
            waitingForResponse.set("ConnectedConsumerTransport", () => {
              success();
              console.log("Consumer transport connected");
            });
          });

          for (const producer of producers) {
            await new Promise((resolve) => {
              sendMessage(ws, {
                action: "Consume",
                producerId: producer.id as ProducerId,
              });

              waitingForResponse.set(
                "Consumed",
                async (consumerOptions: ConsumerOptions) => {
                  const consumer = await (
                    consumerTransport as Transport
                  ).consume(consumerOptions);

                  console.log(`${consumer.kind} consumer created: `, consumer);

                  sendMessage(ws, {
                    action: "ConsumerResume",
                    id: consumer.id as ConsumerId,
                  });

                  receiveMediaStream.addTrack(consumer.track);
                  receivePreview.srcObject = receiveMediaStream;
                  resolve(undefined);
                }
              );
            });
          }
          break;
        default:
          const callback = waitingForResponse.get(decodedMessage.action);

          if (callback) {
            waitingForResponse.delete(decodedMessage.action);
            callback(decodedMessage);
          } else {
            console.error("Recived unexpected message", decodedMessage);
          }
      }
    };
  }

  ws.onerror = console.error;
};

init();
