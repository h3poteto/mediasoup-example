// Refs: https://github.com/versatica/mediasoup-client-aiortc/blob/6a1e648f1fe8fbd2a29019af66c90a79bc043da5/test/test.js#L595
import { Device } from "mediasoup-client";
import { createWorker } from "mediasoup-client-aiortc";
import * as WebSocket from "ws";
import moment from "moment";
let producerTransport;
let consumerTransport;
let selfProducer;
const sendMessage = (client, message) => {
    client.send(JSON.stringify(message));
};
const init = (url) => {
    const client = new WebSocket.WebSocket(url);
    client.onerror = console.error;
    return client;
};
const handleSocket = (client, device) => {
    const waitingForResponse = new Map();
    client.onclose = async (message) => {
        console.log("WebSocket connection is closed because: ", message);
    };
    client.onerror = async (err) => {
        console.error("WebSocket connections has an error: ", err);
    };
    client.on("message", async (data, isBinary) => {
        const message = isBinary ? data : data.toString();
        if (typeof message !== "string") {
            console.error("Message is not string: ", message);
            return;
        }
        const decodedMessage = JSON.parse(message);
        switch (decodedMessage.action) {
            case "Init":
                await device.load({
                    routerRtpCapabilities: decodedMessage.routerRtpCapabilities,
                });
                sendMessage(client, {
                    action: "Init",
                });
                consumerTransport = device.createRecvTransport(decodedMessage.consumerTransportOptions);
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
                producerTransport = device.createSendTransport(decodedMessage.producerTransportOptions);
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
                    waitingForResponse.set("SelfProduced", ({ id }) => {
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
                const consumerOptions = {
                    id: decodedMessage.id,
                    dataProducerId: decodedMessage.dataProducerId,
                    sctpStreamParameters: decodedMessage.sctpStreamParameters,
                    label: decodedMessage.label,
                    protocol: decodedMessage.protocol,
                };
                console.debug("ConsumerOptions: ", consumerOptions);
                const dataConsumer = await consumerTransport.consumeData(consumerOptions);
                console.log(`data consumer created for producer: `, decodedMessage.id);
                dataConsumer.on("message", (message) => {
                    const now = moment();
                    const data = JSON.parse(message);
                    console.debug("received message: ", message);
                    // Do not calculate self producer
                    if (data.producerId === (selfProducer === null || selfProducer === void 0 ? void 0 : selfProducer.id)) {
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
                }
                else {
                    console.error("Recived unexpected message", decodedMessage);
                }
                break;
        }
    });
};
const worker = await createWorker({ logLevel: "debug" });
const device = new Device({ handlerFactory: worker.createHandlerFactory() });
const ws = init("ws://localhost:3000/ws");
handleSocket(ws, device);
const keyevents = async () => {
    return new Promise((resolve) => process.stdin.once("data", () => {
        if (selfProducer) {
            console.log("sending message");
            // Refs: https://mediasoup.org/documentation/v3/mediasoup/api/#dataProducer-send
            const data = {
                producerId: selfProducer.id,
                startTime: moment(),
            };
            selfProducer.send(JSON.stringify(data));
        }
        resolve(null);
    }));
};
process.on("SIGINT", () => {
    process.exit(0);
});
while (true) {
    console.log("Please press Enter key");
    await keyevents();
}
//# sourceMappingURL=index.js.map