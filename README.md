# ao-chat-websocket-proxy

This is a fairly simple TCP <-> Websocket proxy for the Anarchy Online chat servers.

## Configuration

Configuration is done via environment variables, notably:

* `RUST_LOG` will set the log level (defaults to "info")
* `PORT` will set the Websocket server port (defaults to 7777)
* `CHAT_SERVER_ADDR` will set the address of the chat server to connect to (defaults to "chat.d1.funcom.com:7105")

## Running

Running is as simple as downloading a binary from CI runs or the latest release and executing it, optionally configuring as mentioned before.
