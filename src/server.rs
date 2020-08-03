// Server invokes handler.

use crate::handler::Handler;
use std::net::UdpSocket;

// pub struct DNSServer<'a> {
//     handler: Handler,
//     addr: &'a str,
// }

// impl DNSServer<'_> {
//     pub fn new(addr: &'static str) -> Self {
//         Self {
//             handler: Handler::new(),
//             addr,
//         }
//     }

//     pub async fn listen(self) -> Result<(), io::Error> {
//         let socket = UdpSocket::bind(&self.addr).await?;
//         println!("Listening on {}", self.addr);

//         loop {
//             let mut buf = [0; 1024];
//             let (bytes_read, src) = socket.recv_from(&mut buf).await?;

//             let response = match self.handler.handle(&buf[..bytes_read]) {
//                 Ok(resp) => resp,
//                 Err(err) => Err(err),
//             };

//             let response_bytes = response.serialize();
//             match socket.send_to(&response_bytes, src).await? {
//                 Ok(bytes_written) => println!("bytes written: {}", bytes_written),
//                 Err(err) => eprintln!("failed to send back response : {}", err),
//             };
//         }
//     }
// }

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
