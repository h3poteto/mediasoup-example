import * as WebSocket from "ws";
import { Device } from "mediasoup-client";
import { RtpCapabilities } from "mediasoup-client/lib/RtpParameters";
import {
  DtlsParameters,
  Transport,
  TransportOptions,
} from "mediasoup-client/lib/Transport";
import { DataConsumerOptions } from "mediasoup-client/lib/DataConsumer";
import { SctpStreamParameters } from "mediasoup-client/lib/SctpParameters";
import * as moment from "moment";
import { DataProducer } from "mediasoup-client/lib/DataProducer";

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

type LatencyData = {
  producerId: DataProducerId;
  startTime: moment.Moment;
};

let producerTransport: Transport | undefined;
let consumerTransport: Transport | undefined;
let selfProducer: DataProducer | undefined;

const sendMessage = (client: WebSocket, message: ClientMessage) => {
  client.send(JSON.stringify(message));
};

const init = (url: string): WebSocket => {
  const client = new WebSocket(url);

  client.onerror = console.error;
  return client;
};

const handleSocket = (client: WebSocket, device: Device) => {
  const waitingForResponse: Map<ServerMessage["action"], Function> = new Map();

  client.onclose = async (message) => {
    console.log("WebSocket connection is closed because: ", message);
  };
  client.onerror = async (err) => {
    console.error("WebSocket connections has an error: ", err);
  };
  client.on("message", async (data: WebSocket.Data, isBinary: boolean) => {
    const message = isBinary ? data : data.toString();
    if (typeof message !== "string") {
      console.error("Message is not string: ", message);
      return;
    }
    const decodedMessage: ServerMessage = JSON.parse(message);

    switch (decodedMessage.action) {
      case "Init":
        await device.load({
          routerRtpCapabilities: decodedMessage.routerRtpCapabilities,
        });

        sendMessage(client, {
          action: "Init",
        });

        consumerTransport = device.createRecvTransport(
          decodedMessage.consumerTransportOptions
        );

        consumerTransport.on("connect", ({ dtlsParameters }, success) => {
          sendMessage(client, {
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
            sendMessage(client, {
              action: "ConnectProducerTransport",
              dtlsParameters,
            });
            waitingForResponse.set("ConnectedProducerTransport", () => {
              success();
              console.log("Producer transport connected");
            });
          })
          .on("producedata", ({ sctpStreamParameters }, success) => {
            sendMessage(client, {
              action: "Produce",
              sctpStreamParameters,
            });
            waitingForResponse.set("SelfProduced", ({ id }: { id: string }) => {
              success({ id });
            });
          });

        selfProducer = await producerTransport.produceData();
        break;
      case "AlreadyProduced":
        console.debug("AlreadyProduced: ", decodedMessage);
        const ids = decodedMessage.ids;
        ids.forEach((id) => {
          sendMessage(client, {
            action: "Consume",
            dataProducerId: id,
          });
        });
        break;
      case "Produced":
        console.debug("Produced: ", decodedMessage);
        const id = decodedMessage.id;
        sendMessage(client, {
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
        const dataConsumer = await (consumerTransport as Transport).consumeData(
          consumerOptions
        );
        console.log(`data consumer created for producer: `, decodedMessage.id);
        dataConsumer.on("message", (message) => {
          const now = moment();
          const data = JSON.parse(message) as LatencyData;
          console.debug("received message: ", message);
          // Do not calculate self producer
          if (data.producerId === selfProducer?.id) {
            return;
          }
          console.log("latency: ", now.diff(data.startTime));
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
  });
};

const device = new Device();

const ws = init("http://localhost:3001/ws");
handleSocket(ws, device);

const keyevents = async () => {
  process.stdin.setRawMode(true);
  return new Promise((resolve) =>
    process.stdin.once("data", () => {
      process.stdin.setRawMode(false);
      if (selfProducer) {
        console.log("sending message");
        // Refs: https://mediasoup.org/documentation/v3/mediasoup/api/#dataProducer-send
        const data: LatencyData = {
          producerId: selfProducer.id as DataProducerId,
          startTime: moment(),
        };
        selfProducer.send(JSON.stringify(data));
      }
      resolve(null);
    })
  );
};

(async () => {
  console.log("Waiting keyboard event");
  await keyevents();
})().then(process.exit());
