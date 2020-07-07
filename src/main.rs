// Baby steps

mod business;
use crate::business::models;
use std::net::UdpSocket;

fn main() {
    let addr = "127.0.0.1:53";
    let socket = UdpSocket::bind(&addr).expect("Failed to bind.");
    println!("ping/pong server binded to {}", addr);

    let resolver_addr = "192.168.0.16:8888";
    let resolver_socket = UdpSocket::bind(resolver_addr).expect("Failed to bind resolver socket");

    loop {
        let mut buf = [0; 512];
        println!("waiting to receive data...");
        let (amt, src) = socket
            .recv_from(&mut buf)
            .expect("failed to receive data from socket");

        let buf = &mut buf[..amt];
        let query: DNSQuery = DNSQuery::transform_to(buf);
        println!("{:#?}", query);

        let mut resolver_buf = [0; 512];
        resolver_socket
            .send_to(buf, "1.1.1.1:53")
            .expect("failed to write data to resolver socket");
        let (size, _) = resolver_socket
            .recv_from(&mut resolver_buf)
            .expect("failed to receive data from resolver socket");
        let resolver_buf = &mut resolver_buf[..size];
        let resolved_dns_query = DNSQuery::transform_to(&resolver_buf);
        println!("resolved buf: size={} {:#?}", size, resolved_dns_query);

        let written_bytes = socket
            .send_to(resolver_buf, &src)
            .expect("failed to send data to src socket");
        println!("writen bytes={}", written_bytes);
    }
}
