use crate::business::models::{DNSQuery, DNSQueryResponse};
use std::net::{SocketAddr};

use tokio::sync::oneshot::Sender;

#[derive(Debug)]
pub struct ReactorQuery {
    pub query: DNSQuery,
    pub peer_addr: SocketAddr,
    pub tx: Sender<ReactorResponse>
}

#[derive(Debug)]
pub struct ReactorResponse {
    pub response: DNSQueryResponse
}
