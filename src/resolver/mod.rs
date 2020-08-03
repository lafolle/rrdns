use crate::business::models::{
    DNSQuery, DNSQueryHeaderSection, DNSQueryResponse, DNSQuestionQuery, OpCode, QClass, QType,
    RRSet, ResponseCode, Type,
};
use rand::prelude::*;
use itertools::all;
use std::net::UdpSocket;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod cache;
use crate::business::models::ResourceRecord;
use cache::InMemoryCache;

mod zone;
use zone::parent_zone;

// https://tools.ietf.org/html/rfc1034 5
pub struct Resolver {
    socket: UdpSocket,
    cache: InMemoryCache, // TODO: use "Cache" Trait.
}

impl Resolver {
    pub fn new() -> Self {
        let resolver_addr = "0.0.0.0:9999";
        let resolver_socket =
            UdpSocket::bind(resolver_addr).expect("Failed to bind resolver socket");
        Self {
            socket: resolver_socket,
            cache: InMemoryCache::new(),
        }
    }

    pub fn resolve(&mut self, query: &DNSQuery) -> Result<DNSQueryResponse, &'static str> {
        println!(
            "resolve: {} {:#?}",
            query.questions[0].qname, query.questions[0].qtype
        );
        if let Some(response) = self.resolve_from_cache(query) {
            println!(
                "resolve:in_cache: {} {:?}",
                query.questions[0].qname, query.questions[0].qtype
            );
            return response;
        }
        println!(
            "resolve:not_in_cache: {} {:?}",
            query.questions[0].qname, query.questions[0].qtype
        );
        self.resolve_from_name_servers(query)
    }

    fn resolve_from_name_servers(
        &mut self,
        query: &DNSQuery,
    ) -> Result<DNSQueryResponse, &'static str> {
        let domain = &query.questions[0].qname;
        let (name_servers_result, is_grand_parent_ns) = self.fetch_name_servers(domain);
        let name_servers = name_servers_result?;
        if is_grand_parent_ns {
            name_servers.iter().for_each(|rr| {
                let mut new_rr = rr.clone();
                new_rr.name = domain.to_string();
                self.cache.insert2(&new_rr);
            });
        }
        if query.questions[0].qtype == QType::NS {
            let mut query_of_response = query.clone();
            query_of_response.header.is_query = false;
            query_of_response.header.answers_count = name_servers.len() as u16;
            query_of_response.header.is_authoritative_answer = false;
            return Ok(DNSQueryResponse{
                query: query_of_response,
                answers: name_servers,
                authority: vec![],
                additional: vec![],
            });
        }
        self.resolve_from_authority(&query, &name_servers)
    }

    fn resolve_from_cache(
        &mut self,
        query: &DNSQuery,
    ) -> Option<Result<DNSQueryResponse, &'static str>> {
        let domain = &query.questions[0].qname;
        let qtype = &query.questions[0].qtype;
        if let Some(answers) = self.cache.get(domain, qtype) {
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

    fn fetch_name_servers(&mut self, domain: &str) -> (Result<RRSet, &'static str>, bool) {
        if let Some(name_servers) = self.cache.get(&domain, &QType::NS) {
            println!("fetch_name_servers:in_cache: {}", domain);
            return (Ok(name_servers), false);
        }
        println!("fetch_name_servers:not_in_cache: {} {:?}", domain, QType::NS);

        // Ask parent name servers for NS of "domain".
        let parent_zone = parent_zone(domain);
        let parent_ns_query = self.build_query(parent_zone, QType::NS);
        let parent_ns_query_response = match self.resolve(&parent_ns_query){
            Ok(response) => response,
            Err(err) => return (Err(err), false)
        };
        let parent_ns_records = if parent_ns_query_response.answers.len() > 0 {
            parent_ns_query_response.answers
        } else {
            parent_ns_query_response.authority
        };

        let ns_query = self.build_query(domain.to_string(), QType::NS);
        match self.resolve_from_authority(&ns_query, &parent_ns_records) {
            Ok(response) => {
                if response.answers.len() > 0 {
                    if all(response.answers.iter(), |rr| rr.r#type.to_qtype() == QType::NS) {
                        return (Ok(response.answers), false);
                    } else {
                        return (Ok(parent_ns_records), true);
                    }
                }
                if all(response.authority.iter(), |rr| rr.r#type.to_qtype() == QType::NS) {
                    return (Ok(response.authority), false);
                } else {
                    return (Ok(parent_ns_records), true);
                }
            }
            Err(err) => (Err(err), false),
        }
    }

    fn resolve_from_authority(
        &mut self,
        query: &DNSQuery,
        ns_records: &RRSet,
    ) -> Result<DNSQueryResponse, &'static str> {
        println!(
            "resolve_from_authority: {} {:?}",
            query.questions[0].qname, query.questions[0].qtype
        );
        // Get A record for authority server.
        // Sometimes only some of the records may be present in cache
        for authority_server_record in ns_records {
            if let Type::NS(name_server) = &authority_server_record.r#type {
                if let Some(list_of_ipv4s) = self.cache.get(&name_server, &QType::A) {
                    return self.request(&query, list_of_ipv4s);
                } else if let Some(list_of_ipv6s) = self.cache.get(&name_server, &QType::AAAA) {
                    return self.request(&query, list_of_ipv6s);
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
                if let Ok(_) = self.resolve(&a_query) {
                    if let Some(ip_records) = self.cache.get(&dotted_name_server, &QType::A) {
                        return self.request(&query, ip_records);
                    }
                }
            }
        }

        Err("no ip for name servers in cache")
    }

    fn request(
        &mut self,
        query: &DNSQuery,
        ip_records: Vec<ResourceRecord>,
    ) -> Result<DNSQueryResponse, &'static str> {
        for rr in &ip_records {
            let socket_server_addr: SocketAddr = match rr.r#type {
                Type::A(ip4) => SocketAddr::new(IpAddr::V4(ip4), 53),
                Type::AAAA(ip6) => SocketAddr::new(IpAddr::V6(ip6), 53),
                _ => panic!("This should not happen."),
            };

            // println!("query: {:#?} {}", query, socket_server_addr);

            let wire_data = query.serialize();
            println!("-->");
            self.socket
                .send_to(wire_data.as_slice(), socket_server_addr)
                .unwrap();
            println!("<-->");
            let mut response_data = [0; 1024];
            let (read_bytes, _) = self.socket.recv_from(&mut response_data).expect("error");
            println!("<--");
            let response_data = &response_data[..read_bytes];
            let response = DNSQueryResponse::deserialize(response_data);

            // TODO: Replace with idiomatic error handling.
            assert_eq!(query.header.id, response.query.header.id);

            // Update cache.
            if response.query.header.response_code == ResponseCode::NoError {
                self.update_cache(&response);
            }

            return Ok(response);
        }

        Err("request failed")
    }

    fn update_cache(&mut self, response: &DNSQueryResponse) {
        let answers_iter = response.answers.iter();
        let authority_iter = response.authority.iter();
        let additional_iter = response.additional.iter();

        answers_iter
            .chain(authority_iter.chain(additional_iter))
            .for_each(|rr| self.cache.insert2(&rr));
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
