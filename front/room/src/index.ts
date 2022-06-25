// https://www.alpha.co.jp/blog/202205_02
import { Device } from "mediasoup-client";
import { RtpCapabilities } from "mediasoup-client/lib/RtpParameters";
import {
  DtlsParameters,
  Transport,
  TransportOptions,
} from "mediasoup-client/lib/Transport";
import { DataConsumerOptions } from "mediasoup-client/lib/DataConsumer";
import { SctpStreamParameters } from "mediasoup-client/lib/SctpParameters";

type Brand<K, T> = K & { __brand: T };

type DataConsumerId = Brand<string, "DataConsumerId">;
type DataProducerId = Brand<string, "DataProducerId">;

interface ServerInit {
  action: "Init";
  consumerTransportOptions: TransportOptions;
  producerTransportOptions: TransportOptions;
  routerRtpCapabilities: RtpCapabilities;
}

interface ServerAlreadyProduced {
  action: "AlreadyProduced";
  ids: Array<DataProducerId>;
}

interface ServerConnectedProducerTransport {
  action: "ConnectedProducerTransport";
}

interface ServerProduced {
  action: "Produced";
  id: DataProducerId;
}

interface ServerSelfProduced {
  action: "SelfProduced";
  id: DataProducerId;
}

interface ServerConnectedConsumerTransport {
  action: "ConnectedConsumerTransport";
}

interface ServerConsumed {
  action: "Consumed";
  id: DataConsumerId;
  dataProducerId: DataProducerId;
  sctpStreamParameters: SctpStreamParameters;
  label: string;
  protocol: string;
}

type ServerMessage =
  | ServerInit
  | ServerAlreadyProduced
  | ServerConnectedProducerTransport
  | ServerProduced
  | ServerSelfProduced
  | ServerConnectedConsumerTransport
  | ServerConsumed;

interface ClientInit {
  action: "Init";
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
  sctpStreamParameters: SctpStreamParameters;
}

interface ClientConsume {
  action: "Consume";
  dataProducerId: DataProducerId;
}

type ClientMessage =
  | ClientInit
  | ClientConnectProducerTransport
  | ClientProduce
  | ClientConnectConsumerTransport
  | ClientConsume;

const sendMessage = (ws: WebSocket, message: ClientMessage) => {
  ws.send(JSON.stringify(message));
};

const init = async () => {
  const sendButton = document.querySelector("#send") as HTMLButtonElement;

  const ws = new WebSocket("ws://localhost:3000/ws");

  const device = new Device();
  let producerTransport: Transport | undefined;
  let consumerTransport: Transport | undefined;

  {
    const waitingForResponse: Map<ServerMessage["action"], Function> =
      new Map();

    ws.onclose = async (message) => {
      console.log("WebSocket connection is closed because: ", message);
    };
    ws.onerror = async (err) => {
      console.error("WebSocket connections has an error: ", err);
    };

    ws.onmessage = async (message) => {
      const decodedMessage: ServerMessage = JSON.parse(message.data);

      switch (decodedMessage.action) {
        case "Init":
          await device.load({
            routerRtpCapabilities: decodedMessage.routerRtpCapabilities,
          });

          sendMessage(ws, {
            action: "Init",
          });

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
            .on("producedata", ({ sctpStreamParameters }, success) => {
              sendMessage(ws, {
                action: "Produce",
                sctpStreamParameters,
              });
              waitingForResponse.set(
                "SelfProduced",
                ({ id }: { id: string }) => {
                  success({ id });
                }
              );
            });

          const dataProducer = await producerTransport.produceData();
          sendButton.onclick = async () => {
            // Refs: https://mediasoup.org/documentation/v3/mediasoup/api/#dataProducer-send
            console.log("sending message");
            dataProducer.send("sample message");
          };

          break;
        case "AlreadyProduced":
          console.debug("AlreadyProduced: ", decodedMessage);
          const ids = decodedMessage.ids;
          ids.forEach((id) => {
            sendMessage(ws, {
              action: "Consume",
              dataProducerId: id,
            });
          });
          break;
        case "Produced":
          console.debug("Produced: ", decodedMessage);
          const id = decodedMessage.id;
          sendMessage(ws, {
            action: "Consume",
            dataProducerId: id,
          });
          break;
        case "Consumed":
          console.debug("Consumed: ", decodedMessage);
          if (consumerTransport === undefined) {
            console.error("consumerTransport is undefined");
            break;
          }
          const consumerOptions: DataConsumerOptions = {
            id: decodedMessage.id,
            dataProducerId: decodedMessage.dataProducerId,
            sctpStreamParameters: decodedMessage.sctpStreamParameters,
            label: decodedMessage.label,
            protocol: decodedMessage.protocol,
          };
          console.debug("ConsumerOptions: ", consumerOptions);
          const dataConsumer = await (
            consumerTransport as Transport
          ).consumeData(consumerOptions);
          console.log(
            `data consumer created for producer: `,
            decodedMessage.id
          );
          dataConsumer.on("message", (message) => {
            console.log("received message: ", message);
          });
          break;
        default:
          const callback = waitingForResponse.get(decodedMessage.action);

          if (callback) {
            waitingForResponse.delete(decodedMessage.action);
            callback(decodedMessage);
          } else {
            console.error("Recived unexpected message", decodedMessage);
          }
          break;
      }
    };
  }

  ws.onerror = console.error;
};

init();
