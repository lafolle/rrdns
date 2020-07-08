use crate::business::models::DNSQuery;
use std::net::UdpSocket;

pub struct Resolver {
    socket: UdpSocket,
}

impl Resolver {
    pub fn new() -> Resolver {
        let resolver_addr = "192.168.0.16:9999";
        let resolver_socket =
            UdpSocket::bind(resolver_addr).expect("Failed to bind resolver socket");
        Resolver {
            socket: resolver_socket,
        }
    }

    pub fn resolve(&self, query: DNSQuery, response_buf: &mut [u8]) -> usize {
        // this does not work if internet is off.

        println!("{:#?}", query);

        // Send request.
        self.socket
            .send_to(query.buf.iter().as_slice(), "1.1.1.1:53")
            .expect("failed to write data to resolver socket");

        // Receive response.
        let (size, _) = self
            .socket
            .recv_from(response_buf)
            .expect("failed to receive data from resolver socket");
        let resolved_dns_query = DNSQuery::transform_to_wire_format(&response_buf[..size]);
        println!("resolved buf: size={} {:#?}", size, resolved_dns_query);

        size
    }
}
