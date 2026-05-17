use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use std::error::Error;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Sender, channel};
use tokio_websockets::{Message, ServerBuilder, WebSocketStream};

async fn handle_connection(
    addr: SocketAddr,
    mut ws_stream: WebSocketStream<TcpStream>,
    // Modified to accept a tuple so we know who sent the message
    bcast_tx: Sender<(String, SocketAddr)>,
) -> Result<(), Box<dyn Error + Send + Sync>> {

    // Subscribe to the broadcast channel
    let mut bcast_rx = bcast_tx.subscribe();

    loop {
        tokio::select! {
            // 1. Receive messages from this client and broadcast them to the channel
            incoming = ws_stream.next() => {
                match incoming {
                    Some(Ok(msg)) => {
                        if let Some(text) = msg.as_text() {
                            println!("From client {addr:?} {text:?}");
                            // Send the text along with the sender's SocketAddr
                            let _ = bcast_tx.send((text.to_string(), addr));
                        }
                    }
                    Some(Err(e)) => return Err(e.into()),
                    None => return Ok(()), // Client disconnected
                }
            }
            
            // 2. Receive messages from the broadcast channel and send to this client
            msg_result = bcast_rx.recv() => {
                match msg_result {
                    Ok((msg_text, sender_addr)) => {
                        // OPTIONAL TASK: Only send if the sender is NOT the current client
                        if sender_addr != addr {
                            let formatted_msg = format!("{}: {}", sender_addr, msg_text);
                            ws_stream.send(Message::text(formatted_msg)).await?;
                        }
                    }
                    Err(_) => {
                        // This happens if the receiver lagged too far behind
                        // or if there are no senders left.
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Change the channel type to hold a tuple of (Message, Sender Address)
    let (bcast_tx, _) = channel::<(String, SocketAddr)>(16);

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("listening on port 8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from {addr:?}");
        let bcast_tx = bcast_tx.clone();
        
        tokio::spawn(async move {
            // Wrap the raw TCP stream into a websocket.
            match ServerBuilder::new().accept(socket).await {
                Ok(ws_stream) => {
                    if let Err(e) = handle_connection(addr, ws_stream, bcast_tx).await {
                        eprintln!("Connection error from {addr:?}: {e}");
                    }
                }
                Err(e) => eprintln!("Failed to accept websocket from {addr:?}: {e}"),
            }
        });
    }
}