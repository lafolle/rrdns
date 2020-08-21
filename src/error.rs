use crate::business::models::DNSQueryResponse;
use std::io::Error;

#[derive(Debug)]
pub enum FetchError {
    QueryError(DNSQueryResponse),
    NetworkError(Error),

    // TODO: This case should not be an error.
    InfiniteRecursionError(String),

    // VerificationError(String),
    NoIPError(String),
}
