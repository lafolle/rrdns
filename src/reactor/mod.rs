use crate::business::models::DNSQueryResponse;
use log::{info, error};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{channel, Receiver, Sender};

pub mod cmd;
use cmd::{ReactorQuery, ReactorResponse};

pub struct Reactor {
    addr: &'static str,
    rx: Receiver<ReactorQuery>,
}

impl Reactor {
    pub fn new(addr: &'static str) -> Sender<ReactorQuery> {
        let (tx, rx) = channel(10);

        let reactor = Reactor { addr, rx };

        tokio::spawn(reactor.run());

        tx
    }

    pub async fn run(mut self) {
        let addr = SocketAddr::new(IpAddr::V4(self.addr.parse::<Ipv4Addr>().unwrap()), 34254);
        let mut socket = UdpSocket::bind(addr)
            .await
            .expect(format!("reactor: could not bind to address {}", addr).as_str());

        let mut registry = HashMap::new();

        info!("Reactor is binded on address {}", addr);
        loop {
            let mut read_buf = [0 as u8; 1024];

            tokio::select! {

                Some(cmd) = self.rx.recv() => {
                    let wire_data = cmd.query.serialize();
                    match socket.send_to(wire_data.as_slice(), cmd.peer_addr).await {
                        Ok(written_bytes) => {
                            info!("reactor: written {} bytes", written_bytes);
                            registry.insert(cmd.query.header.id, cmd);
                        },
                        Err(err) => {
                            // BUG: return error to the resolver. cmd.tx.send();
                            error!("reactor: error={} addr={}", err, addr)
                        }
                    };
                },

                Ok(read_bytes_count) = socket.recv(&mut read_buf) => {
                    let response_data = &read_buf[..read_bytes_count];
                    let response = DNSQueryResponse::deserialize(response_data);

                    if let Some(cmd) = registry.remove(&response.query.header.id) {
                        let reactor_response = ReactorResponse {
                            response
                        };
                        if cmd.tx.is_closed() {
                            panic!("reactor: oneshot receiver is closed");
                        }
                        cmd.tx.send(reactor_response).unwrap();
                    } else {
                        // This should never happen.
                        error!("\"{}\" is missing in registry", response.query.header.id);
                    }
                }

            }
        }
    }
}
