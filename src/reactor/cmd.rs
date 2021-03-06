use crate::business::models::{DNSQuery, DNSQueryResponse};
use crate::error::FetchError;
use std::net::SocketAddr;

use tokio::sync::oneshot::Sender;

#[derive(Debug)]
pub struct ReactorQuery {
    pub query: DNSQuery,
    pub peer_addr: SocketAddr,
    pub respond_tx: Sender<Result<ReactorResponse, FetchError>>,
}

#[derive(Debug)]
pub struct ReactorResponse {
    pub response: DNSQueryResponse,
}
