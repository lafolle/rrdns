use crate::business::models::{
    DNSQuery, DNSQueryHeaderSection, DNSQueryResponse, DNSQuestionQuery, OpCode, QClass, QType,
    RRSet, ResponseCode, Type,
};
use log::{info, error};
use itertools::all;
use rand::prelude::*;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

mod cache;
use crate::business::models::ResourceRecord;
use crate::reactor::cmd::ReactorQuery;
use crate::reactor::Reactor;
use async_recursion::async_recursion;
use cache::{Cache, InMemoryCache};
use tokio::sync::mpsc::Sender;

mod zone;
use zone::parent_zone;

// https://tools.ietf.org/html/rfc1034 5
// Resolver is not thread safe and needs to be accessed via mutext because
// firstly, a socket is not thread safe and multiple threads writing to socket
// would corrupt the kernet buffer and secondly cache is not thead safe as well.
pub struct Resolver {
    reactor_tx: Sender<ReactorQuery>,
    cache: Arc<Mutex<dyn Cache + Send>>,
}

impl Resolver {
    pub fn new() -> Self {
        let reactor_addr = "0.0.0.0";
        let reactor_tx = Reactor::new(reactor_addr);

        Self {
            reactor_tx,
            cache: Arc::new(Mutex::new(InMemoryCache::new())),
        }
    }

    #[async_recursion]
    pub async fn resolve(&self, query: &DNSQuery) -> Result<DNSQueryResponse, &'static str> {
        info!(
            "{} resolve: {} {:#?}",
            query.header.id, query.questions[0].qname, query.questions[0].qtype
        );
        if let Some(response) = self.resolve_from_cache(query) {
            info!(
                "{} resolve:in_cache: {} {:?}",
                query.header.id, query.questions[0].qname, query.questions[0].qtype
            );
            return response;
        }
        info!(
            "{} resolve:not_in_cache: {} {:?}",
            query.header.id, query.questions[0].qname, query.questions[0].qtype
        );
        self.resolve_from_name_servers(query).await
    }

    async fn resolve_from_name_servers(
        &self,
        query: &DNSQuery,
    ) -> Result<DNSQueryResponse, &'static str> {
        let domain = &query.questions[0].qname;
        let (name_servers_result, is_grand_parent_ns) = self.fetch_name_servers(domain).await;
        let name_servers = name_servers_result?;
        if is_grand_parent_ns {
            name_servers.iter().for_each(|rr| {
                let mut new_rr = rr.clone();
                new_rr.name = domain.to_string();
                let mut cache = self.cache.lock().unwrap();
                cache.insert2(&new_rr);
            });
        }
        if query.questions[0].qtype == QType::NS {
            let mut query_of_response = query.clone();
            query_of_response.header.is_query = false;
            query_of_response.header.answers_count = name_servers.len() as u16;
            query_of_response.header.is_authoritative_answer = false;
            return Ok(DNSQueryResponse {
                query: query_of_response,
                answers: name_servers,
                authority: vec![],
                additional: vec![],
            });
        }
        self.resolve_from_authority(&query, &name_servers).await
    }

    fn resolve_from_cache(
        &self,
        query: &DNSQuery,
    ) -> Option<Result<DNSQueryResponse, &'static str>> {
        let domain = &query.questions[0].qname;
        let qtype = &query.questions[0].qtype;
        let mut cache = self.cache.lock().unwrap();
        if let Some(answers) = cache.get(domain, qtype) {
            let mut query_of_response = query.clone();
            query_of_response.header.is_query = false;
            query_of_response.header.answers_count = answers.len() as u16;
            query_of_response.header.is_authoritative_answer = false;
            return Some(Ok(DNSQueryResponse {
                query: query_of_response,
                answers: answers,
                authority: vec![],
                additional: vec![],
            }));
        }
        None
    }

    async fn fetch_name_servers(&self, domain: &str) -> (Result<RRSet, &'static str>, bool) {
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(name_servers) = cache.get(&domain, &QType::NS) {
                info!("fetch_name_servers:in_cache: {}", domain);
                return (Ok(name_servers), false);
            }
        }
        info!(
            "fetch_name_servers:not_in_cache: {} {:?}",
            domain,
            QType::NS
        );

        // Ask parent name servers for NS of "domain".
        let parent_zone = parent_zone(domain);
        let parent_ns_query = self.build_query(parent_zone, QType::NS);
        let parent_ns_query_response = match self.resolve(&parent_ns_query).await {
            Ok(response) => response,
            Err(err) => return (Err(err), false),
        };
        let parent_ns_records = if parent_ns_query_response.answers.len() > 0 {
            parent_ns_query_response.answers
        } else {
            parent_ns_query_response.authority
        };

        let ns_query = self.build_query(domain.to_string(), QType::NS);
        match self
            .resolve_from_authority(&ns_query, &parent_ns_records)
            .await
        {
            Ok(response) => {
                if response.answers.len() > 0 {
                    if all(response.answers.iter(), |rr| {
                        rr.r#type.to_qtype() == QType::NS
                    }) {
                        return (Ok(response.answers), false);
                    } else {
                        return (Ok(parent_ns_records), true);
                    }
                }
                if all(response.authority.iter(), |rr| {
                    rr.r#type.to_qtype() == QType::NS
                }) {
                    return (Ok(response.authority), false);
                } else {
                    return (Ok(parent_ns_records), true);
                }
            }
            Err(err) => (Err(err), false),
        }
    }

    async fn resolve_from_authority(
        &self,
        query: &DNSQuery,
        ns_records: &RRSet,
    ) -> Result<DNSQueryResponse, &'static str> {
        info!(
            "{} resolve_from_authority: {} {:?}",
            query.header.id, query.questions[0].qname, query.questions[0].qtype
        );
        // Get A record for authority server.
        // Sometimes only some of the records may be present in cache
        for authority_server_record in ns_records {
            if let Type::NS(name_server) = &authority_server_record.r#type {
                let mut A_record = None;
                let mut AAAA_record = None;
                {
                    let mut cache = self.cache.lock().unwrap();
                    A_record = cache.get(&name_server, &QType::A);
                    AAAA_record = cache.get(&name_server, &QType::AAAA);
                }
                if let Some(list_of_ipv4s) = A_record {
                    return self.request(&query, list_of_ipv4s).await;
                } else if let Some(list_of_ipv6s) = AAAA_record {
                    return self.request(&query, list_of_ipv6s).await;
                }
            };
        }

        for authority_server_record in ns_records {
            // Build query to get A/AAAA record for NS.
            if let Type::NS(name_server) = &authority_server_record.r#type {
                let mut dotted_name_server = name_server.clone();
                if !name_server.ends_with(".") {
                    dotted_name_server.push('.');
                }

                let a_query = self.build_query(dotted_name_server.clone(), QType::A);
                if let Ok(_) = self.resolve(&a_query).await {
                    let mut A_record = None;
                    {
                        let mut cache = self.cache.lock().unwrap();
                        A_record = cache.get(&dotted_name_server, &QType::A);
                    }
                    if let Some(ip_records) = A_record {
                        return self.request(&query, ip_records).await;
                    }
                }
            }
        }

        Err("no ip for name servers in cache")
    }

    async fn request(
        &self,
        query: &DNSQuery,
        ip_records: Vec<ResourceRecord>,
    ) -> Result<DNSQueryResponse, &'static str> {
        for rr in &ip_records {
            let socket_server_addr: SocketAddr = match rr.r#type {
                Type::A(ip4) => SocketAddr::new(IpAddr::V4(ip4), 53),
                Type::AAAA(ip6) => SocketAddr::new(IpAddr::V6(ip6), 53),
                _ => panic!("This should not happen."),
            };

            let (tx_oneshot, rx_oneshot) = oneshot::channel();

            let reactor_query = ReactorQuery {
                query: query.clone(),
                peer_addr: socket_server_addr,
                tx: tx_oneshot,
            };

            let mut reactor_tx = self.reactor_tx.clone();
            match reactor_tx.send(reactor_query).await {
                Ok(_) => {
                    match rx_oneshot.await {
                        Ok(reactor_response) => {
                            // Update cache.
                            if reactor_response.response.query.header.response_code
                                == ResponseCode::NoError
                            {
                                self.update_cache(&reactor_response.response);
                            }
                            return Ok(reactor_response.response);
                        }
                        Err(err) => {
                            error!("Failed to receive response on oneshot channel from reactor: err={}", err);
                        }
                    }
                }
                Err(err) => error!("resolver:reactor_send error={}", err),
            };
        }

        Err("request failed")
    }

    fn update_cache(&self, response: &DNSQueryResponse) {
        let answers_iter = response.answers.iter();
        let authority_iter = response.authority.iter();
        let additional_iter = response.additional.iter();

        let mut cache = self.cache.lock().unwrap();
        answers_iter
            .chain(authority_iter.chain(additional_iter))
            .for_each(|rr| cache.insert2(&rr));
    }

    // TODO: use builder pattern.
    fn build_query(&self, qname: String, qtype: QType) -> DNSQuery {
        DNSQuery {
            header: DNSQueryHeaderSection {
                id: random(),
                op_code: OpCode::Query,
                is_query: true,
                is_truncated: false,
                is_authoritative_answer: false,
                is_recursion_available: false,
                is_recursion_desired: true,
                response_code: ResponseCode::NoError,
                questions_count: 1,
                answers_count: 0,
                ns_rr_count: 0,
                additional_rr_count: 0,
            },
            questions: vec![DNSQuestionQuery {
                qname,
                qclass: QClass::IN,
                qtype,
            }],
            additionals: vec![],
        }
    }
}
