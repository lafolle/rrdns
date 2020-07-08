// Server invokes handler.

use crate::handler::Handler;
use std::net::UdpSocket;

pub fn run() {
    let addr = "127.0.0.1:53";
    let socket = UdpSocket::bind(&addr).expect("Failed to bind.");
    println!("ping/pong server binded to {}", addr);

    // Create new handler.
    let handler = Handler::new();

    loop {
        let mut buf = [0; 512];
        println!("waiting to receive data...");
        let (amt, src) = socket
            .recv_from(&mut buf)
            .expect("failed to receive data from socket");
        let buf = &mut buf[..amt];

        let mut response_buf: [u8; 512] = [0; 512];
        let response_size = handler.handle(buf, &mut response_buf);

        let written_bytes = socket
            .send_to(&response_buf[..response_size], &src)
            .expect("failed to send data to src socket");
        println!("writen bytes={}", written_bytes);
    }
}
