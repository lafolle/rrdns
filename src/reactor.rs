use crate::business::models::{DNSQueryResponse, ResponseCode};
use crate::error::FetchError;
use log::{debug, error, info};
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
        debug!("new reactor created/spawned");

        tx
    }

    pub async fn run(mut self) {
        let addr = SocketAddr::new(IpAddr::V4(self.addr.parse::<Ipv4Addr>().unwrap()), 34256);
        let mut socket = match UdpSocket::bind(addr).await {
            Ok(socket) => socket,
            Err(err) => {
                error!("could not bind the reactor to {} because of {}", addr, err);
                std::process::exit(1)
            }
        };

        let mut registry = HashMap::new();

        info!("Reactor is binded to address {}", addr);
        loop {
            let mut read_buf = [0 as u8; 1024];

            tokio::select! {

                Some(cmd) = self.rx.recv() => {
                    let wire_data = cmd.query.serialize();
                    match socket.send_to(wire_data.as_slice(), cmd.peer_addr).await {
                        Ok(written_bytes) => {
                            info!("{} reactor: written {} bytes to addr={}", cmd.query.header.id, written_bytes, cmd.peer_addr);
                            if registry.contains_key(&cmd.query.header.id) {
                                error!("{} key={} exists", cmd.query.header.id, cmd.query.header.id);
                            }
                            registry.insert(cmd.query.header.id, cmd);
                        },
                        Err(err) => {
                            error!("reactor: send_to error={} addr={}", err, addr);
                            cmd.respond_tx.send(Err(FetchError::NetworkError(err))).unwrap();
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
                        if cmd.respond_tx.is_closed() {
                            panic!("reactor: oneshot receiver is closed");
                        }

                        if reactor_response.response.query.header.response_code != ResponseCode::NoError {
                            cmd.respond_tx.send(Err(FetchError::QueryError(reactor_response.response))).unwrap();
                            continue;
                        }

                        cmd.respond_tx.send(Ok(reactor_response)).unwrap();
                    } else {
                        // This should never happen.
                        error!("\"{}\" is missing in registry", response.query.header.id);
                    }
                }

            }
        }
    }
}
