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

interface ServerConnectedProducerTransport {
  action: "ConnectedProducerTransport";
}

interface ServerProduced {
  action: "Produced";
  id: DataProducerId;
}

interface ServerConnectedConsumerTransport {
  action: "ConnectedConsumerTransport";
}

interface ServerConsumed {
  action: "Consumed";
  id: DataConsumerId;
}

type ServerMessage =
  | ServerInit
  | ServerConnectedProducerTransport
  | ServerProduced
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
              waitingForResponse.set("Produced", ({ id }: { id: string }) => {
                success({ id });
              });
            });

          const dataProducer = await producerTransport.produceData();
          sendButton.onclick = async () => {
            // Refs: https://mediasoup.org/documentation/v3/mediasoup/api/#dataProducer-send
            console.log("sending message");
            dataProducer.send("sample message");
          };

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

          await new Promise((resolve) => {
            sendMessage(ws, {
              action: "Consume",
              dataProducerId: dataProducer.id as DataProducerId,
            });

            waitingForResponse.set(
              "Consumed",
              async (consumerOptions: DataConsumerOptions) => {
                const dataConsumer = await (
                  consumerTransport as Transport
                ).consumeData(consumerOptions);

                console.log(`data consumer created: `, dataConsumer);

                // Receive data from consumer
                dataConsumer.on("message", (message) => {
                  console.log("received message: ", message);
                });

                resolve(undefined);
              }
            );
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
      }
    };
  }

  ws.onerror = console.error;
};

init();
