// Baby steps
use log::{info, error, debug};
use crate::business::models::DNSQueryResponse;
use crate::handler::Handler;
use clap::{App, Arg};
use std::net::SocketAddr;
use std::panic;
use std::ops::Deref;
use std::sync::Arc;
use tokio;
use tokio::sync::mpsc;

mod business;
mod handler;
mod reactor;
mod resolver;
mod server;

fn init() -> String {
    // Initialize the logger.
    env_logger::init();

    // Set panic
    panic::set_hook(Box::new(|panic_info| {
        let (filename, line) = panic_info
            .location()
            .map(|loc| (loc.file(), loc.line()))
            .unwrap_or(("<unknown>", 0));

        let cause = panic_info.payload().downcast_ref::<String>().map(String::deref);
        let cause = cause.unwrap_or_else(|| 
            panic_info.payload().downcast_ref::<&str>().map(|s|*s).unwrap_or("<cause unknown>"));

        error!("PANIC occured at {}:{} {}", filename, line, cause);
    }));

    // Parse CLI args.
    let matches = App::new("rrdns")
        .version("0.0")
        .author("lafolle")
        .about("Recursive DNS resolver in Rust")
        .arg(
            Arg::with_name("listen")
                .short("l")
                .long("listen")
                .takes_value(true)
                .help("Server will listen for DNS queries on this address"),
        )
        .get_matches();

    matches
        .value_of("listen")
        .unwrap_or("127.0.0.1:8888")
        .to_string()
}

#[tokio::main]
async fn main() {
    let listen_addr = init();

    let handler = Arc::new(Handler::new());

    let addr = listen_addr.parse::<SocketAddr>().unwrap();
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    info!("DNS resolver binded to address {}", listen_addr);
    let (mut socket_rx, mut socket_tx) = socket.split();
    let (response_tx, mut response_rx) = mpsc::channel::<(Result<DNSQueryResponse, &'static str>, SocketAddr)>(5);

    // Read DNS queries from socket.
    let read_handler = tokio::spawn(async move {
        // Client has sent a request.
        let mut buf = [0; 1024];
        while let Ok((bytes_read_count, peer)) = socket_rx.recv_from(&mut buf).await {
            debug!("bytes read count: {}", bytes_read_count);
            let mut dst_buf = vec![0; bytes_read_count];
            dst_buf[..].copy_from_slice(&mut buf[..bytes_read_count]);
            tokio::spawn(process(dst_buf, peer, handler.clone(), response_tx.clone()));
        }
    });

    // Write responses to socket.
    let write_handler = tokio::spawn(async move {
        loop {
            let (response_result, peer) = response_rx.recv().await.unwrap();
            if response_result.is_ok() {
                // BUG: return response with error data set.
                let raw_response = response_result.unwrap().serialize();
                let written_bytes = socket_tx.send_to(&raw_response, &peer).await.unwrap();
                debug!("main: written_bytes={}", written_bytes);
            }
        }
    });

    read_handler.await.unwrap();
    write_handler.await.unwrap();
}

async fn process(
    buf: Vec<u8>, // propagated
    peer: SocketAddr,
    handler: Arc<Handler>,
    mut response_tx: mpsc::Sender<(Result<DNSQueryResponse, &'static str>, SocketAddr)>,
) {
    let response = handler.handle(&buf).await;
    response_tx.send((response, peer)).await.unwrap();
}
