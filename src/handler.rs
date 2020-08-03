use crate::business::models::{DNSQuery, DNSQueryResponse};
use crate::resolver::Resolver;

pub struct Handler {
    pub resolver: Resolver,
}

impl<'a> Handler {
    pub fn new() -> Self {
        Self {
            resolver: Resolver::new(),
        }
    }

    pub fn handle(&mut self, buf: &[u8]) -> Result<DNSQueryResponse, &'static str> {
        let (query, _) = DNSQuery::deserialize(buf);

        let rewritten_query = self.rewrite_query(query);

        self.resolver.resolve(&rewritten_query)
    }

    fn rewrite_query(&self, query: DNSQuery) -> DNSQuery {
        let mut new_query = query.clone();

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
