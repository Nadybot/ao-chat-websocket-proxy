use futures_util::{future::select_all, SinkExt, StreamExt};
use libc::{c_int, sighandler_t, signal, SIGINT, SIGTERM};
use log::{debug, error, info, trace, warn};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    runtime::Builder,
    task::JoinHandle,
};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

use std::{env, io, net::SocketAddr};

pub extern "C" fn handler(_: c_int) {
    std::process::exit(0);
}

unsafe fn set_os_handlers() {
    signal(SIGINT, handler as extern "C" fn(_) as sighandler_t);
    signal(SIGTERM, handler as extern "C" fn(_) as sighandler_t);
}

async fn handle_connection(ws: WebSocketStream<TcpStream>) -> io::Result<()> {
    let (mut ws_tx, mut ws_rx) = ws.split();

    let chat_server_addr =
        env::var("CHAT_SERVER_ADDR").unwrap_or_else(|_| String::from("chat.d1.funcom.com:7105"));
    let ao_socket = TcpStream::connect(&chat_server_addr).await?;
    let (mut ao_rx, mut ao_tx) = ao_socket.into_split();

    debug!("Established chat server connection to {}", chat_server_addr);

    // Dump the websocket messages to the TCP stream
    let send_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            if msg.is_text() || msg.is_binary() {
                let data = msg.into_data();

                trace!("Writing {} bytes to TCP stream", data.len());

                if ao_tx.write_all(&data).await.is_err() {
                    break;
                }
            }
        }

        debug!("Stopped receiving websocket messages");

        Ok(())
    });

    // Dump the TCP messages to the websocket stream
    let receive_task = tokio::spawn(async move {
        let mut header_buffer = vec![0; 4];

        loop {
            // Read the packet header
            ao_rx.read_exact(&mut header_buffer).await?;
            // Header consists of 4 bytes, last 2 are the body length
            let packet_length = u16::from_be_bytes(header_buffer[2..4].try_into().unwrap());

            // Read the rest of the body
            let mut packet_body = vec![0; packet_length as usize];
            ao_rx.read_exact(&mut packet_body).await?;

            // Concentate the buffers
            let full_packet = [header_buffer.clone(), packet_body].concat();

            trace!("Writing {} bytes to websocket stream", full_packet.len());

            // Send it to the websocket
            if ws_tx.send(Message::Binary(full_packet)).await.is_err() {
                break;
            }
        }

        debug!("Stopped receiving TCP messages");

        Ok(())
    });

    let futures: Vec<JoinHandle<io::Result<()>>> = vec![send_task, receive_task];

    let (_, _, others) = select_all(futures).await;

    for future in others {
        future.abort();
    }

    Ok(())
}

async fn serve() -> io::Result<()> {
    let port = env::var("PORT").map_or(7777, |p| p.parse().unwrap());
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();

    let listener = if let Ok(listener) = TcpListener::bind(&addr).await {
        info!("Listening on {:?}", addr);
        listener
    } else {
        error!("Failed to bind to {}", addr);
        return Ok(());
    };

    while let Ok((stream, addr)) = listener.accept().await {
        info!("Incoming connection from {:?}", addr.ip());

        tokio::spawn(async move {
            if let Ok(ws) = accept_async(stream).await {
                handle_connection(ws).await?;
            } else {
                warn!("Websocket upgrade failed, disconnecting");
            }

            Ok::<(), io::Error>(())
        });
    }

    Ok(())
}

fn main() -> io::Result<()> {
    unsafe { set_os_handlers() };

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(serve())
}
