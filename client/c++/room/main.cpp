// https://docs.websocketpp.org/md_tutorials_utility_client_utility_client.html
// https://github.com/zaphoyd/websocketpp/blob/master/examples/echo_client/echo_client.cpp
#include <functional>
#include <iostream>
#include <string>
#include <websocketpp/client.hpp>
#include <websocketpp/config/asio_no_tls_client.hpp>
#include <mediasoupclient/mediasoupclient.hpp>
#include <nlohmann/json.hpp>

using json = nlohmann::json;

typedef websocketpp::client<websocketpp::config::asio_client> client;

typedef websocketpp::config::asio_client::message_type::ptr message_ptr;

static rtc::Thread* networkThread;

namespace ServerMessage {
  struct Init {
    std::string action;
  };

  void to_json(json &j, const Init &d) {
    j = json{{"action", d.action}};
  }

  void from_json(const json &j, Init &d) {
    j.at("action").get_to(d.action);
  }

}

void on_message(client *c, websocketpp::connection_hdl hdl, message_ptr msg) {
  std::cout << "on_message called with hdl: " << hdl.lock().get()
            << "and message: " << msg->get_payload()
            << std::endl;

  json message = json::parse(msg->get_payload());

  std::cout << message["action"] << std::endl;

  /*
  std::string init = "Init";

  // TODO: implement
  switch(message["action"]) {
  case init:
    ServerMessage::Init m = message.get<ServerMessage::Init>();
    break;
  default:
    std::cout << "Receive undefined action: " << message["action"] << std::endl;
    return
    }*/

  /*
  websocketpp::lib::error_code ec;

  c->send(hdl, msg->get_payload(), msg->get_opcode(), ec);
  if (ec) {
    std::cout << "Echo failed because: " << ec.message() << std::endl;
  }
  */
}

class WebsocketHandler {
public:
  WebsocketHandler(std::string uri, mediasoupclient::Device *device) : m_conn(nullptr), m_uri(uri) {
    m_client.clear_access_channels(websocketpp::log::alevel::all);
    m_client.clear_error_channels(websocketpp::log::elevel::all);


    m_client.init_asio();
    m_client.start_perpetual();

    m_device = device;

    m_client.set_message_handler(websocketpp::lib::bind(&on_message, &m_client, websocketpp::lib::placeholders::_1, websocketpp::lib::placeholders::_2));
    m_thread.reset(new websocketpp::lib::thread(&client::run, &m_client));
  }
  ~WebsocketHandler() {
    m_client.stop_perpetual();
    websocketpp::lib::error_code ec;
    if (m_conn != nullptr) {
      std::cout << "closing connection" << std::endl;
      m_client.close(m_conn->get_handle(), websocketpp::close::status::going_away, "Close", ec);
      if (ec) {
        std::cout << "Error closing connection: " << ec.message() << std::endl;
      }
    }

    m_thread->join();
  }
  int connect() {
    websocketpp::lib::error_code ec;
    m_conn = m_client.get_connection(m_uri, ec);
    if (ec) {
      std::cout << "could not create connection because: " << ec.message() << std::endl;
      return 0;
    }
    m_client.connect(m_conn);
    return 0;
  }
  void close(websocketpp::close::status::value code) {
    websocketpp::lib::error_code ec;
    if (m_conn != nullptr) {
      std::cout << "closing connection" << std::endl;
      m_client.close(m_conn->get_handle(), code, "Close", ec);
      if (ec) {
        std::cout << "Error closing connection: " << ec.message() << std::endl;
      } else {
        m_conn = nullptr;
      }
    }
  }
private:
  std::string m_uri;
  client m_client;
  websocketpp::lib::shared_ptr<websocketpp::lib::thread> m_thread;
  client::connection_ptr m_conn;
  mediasoupclient::Device *m_device;
};

int main(int argc, char* argv[]) {

  mediasoupclient::Initialize();
  mediasoupclient::Device *device = new mediasoupclient::Device();

  std::string uri = "ws://localhost:3000/ws";

  if (argc == 2) {
    uri = argv[1];
  }

  WebsocketHandler handler(uri, device);

  handler.connect();
  bool done = false;
  std::string input;



  while(!done) {
    std::cout << "Enter Command\n" << "> ";
    std::getline(std::cin, input);

    if (input == "quit") {
      std::string status = "Quit";
      handler.close(websocketpp::close::status::going_away);

      done = true;
    } else if (input == "help") {
      std::cout
        << "\nCommand List:\n"
        << "help: Display this help text\n"
        << "quit: Exit the program\n"
        << std::endl;
    } else {
      std::cout << "Ungrecognized command" << std::endl;
    }

  }

  return 0;
}
