// Server invokes handler.

use crate::handler::Handler;
use std::net::{UdpSocket, SocketAddr};

pub struct DNSServer {
    handler: Handler,
    addr: SocketAddr,
    // reactor: UDPReactor
}

impl DNSServer {
    pub fn new(addr: &'static str) -> Self {
        Self {
            handler: Handler::new(),
            addr: addr.parse().unwrap(),
        }
    }
}

/*
pub fn run() {
    let addr = "127.0.0.1:53";
    let socket = UdpSocket::bind(&addr).expect("Failed to bind.");
    println!("ping/pong server binded to {}", addr);

    // Create new handler.
    let mut handler: Handler = Handler::new();

    loop {
        // Receive
        let mut buf = [0; 512];

        println!("waiting to receive data...");
        let (amt, src) = socket
            .recv_from(&mut buf)
            .expect("failed to receive data from socket");
        let buf = &mut buf[..amt];

        // Process
        let response = match handler.handle(buf) {
            Ok(resp) => resp,
            Err(err) => {
                panic!(err);
            }
        };
        println!("FINAL response: {:#?}", response);
        let response_buf = response.serialize();

        // Respond
        let written_bytes = socket
            .send_to(&response_buf, &src)
            .expect("failed to send data to src socket");
        println!("writen bytes={}", written_bytes);
    }
}
*/
