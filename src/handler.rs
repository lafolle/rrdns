use crate::business::models::{DNSQuery, DNSQueryResponse, QType};
use crate::error::FetchError;
use crate::resolver::cache::{Store};
use crate::resolver::Resolver;
use log::info;
use rand::prelude::*;
use std::sync::Arc;

// Handler can be called from multiple threads.
pub struct Handler {
    pub resolver: Arc<Resolver>,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            resolver: Arc::new(Resolver::new()),
        }
    }

    pub async fn handle(&self, buf: &[u8]) -> Result<DNSQueryResponse, FetchError> {
        let (query, _) = DNSQuery::deserialize(buf);
        let query_id = query.header.id;
        let query_qname = query.questions[0].qname.clone();
        let query_qtype = query.questions[0].qtype.clone();
        let query_qclass = query.questions[0].qclass.clone();

        let rewritten_query = self.rewrite_query(query);

        match self.resolver.resolve(&rewritten_query).await {
            Ok(mut response) => {
                info!("{}/{} resolved", response.query.header.id, query_id);
                response.query.header.id = query_id;
                response.query.header.is_recursion_available = true;
                return Ok(response);
            }
            Err(err) => {
                match err {
                    FetchError::QueryError(mut response) => {
                        response.query.header.id = query_id;
                        response.query.questions[0].qname = query_qname;
                        response.query.questions[0].qtype = query_qtype;
                        response.query.questions[0].qclass = query_qclass;
                        return Err(FetchError::QueryError(response));
                    }
                    FetchError::NetworkError(err) => {
                        // BUG: Client needs to be told about ISE.  How???  Does the RFC say
                        // anyghing about it?
                        return Err(FetchError::NetworkError(err));
                    }
                    FetchError::InfiniteRecursionError(err) => {
                        return Err(FetchError::InfiniteRecursionError(err));
                    }
                    FetchError::NoIPError(err) => {
                        return Err(FetchError::NoIPError(err));
                    }
                }
            }
        }
    }

    pub fn clone_cache(&self) -> Store {
        self.resolver.clone_cache()
    }

    fn rewrite_query(&self, query: DNSQuery) -> DNSQuery {
        let mut new_query = query.clone();

        // We are going to use our own ids and not rely on ids provided by client to prevent
        // sending answer for Q1 to Q2 where Q1.id = Q2.id is true and Q1 and Q2 come around the
        // same time.  Ideally stronger guarantee is needed,  probably by hashing on ip:port of
        // query if LBs can pass it on to rrdns.  NOTE: Is there better way to solve
        // fast-theread-safe-rand-id-generation???
        let new_id = random();
        if new_id == query.header.id {
            panic!("id collision {}", new_id);
        }
        new_query.header.id = new_id;

        // All domains must be suffixed by ".".  Dig does not add one,  not sure if anyother client
        // adds it.
        if !new_query.questions[0].qname.ends_with('.') {
            new_query.questions[0].qname.push('.');
        }

        // TODO: what does "additionals" contain in query?
        new_query.additionals = vec![];
        new_query.header.additional_rr_count = 0;

        new_query
    }
}
