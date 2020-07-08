use crate::business::models::DNSQuery;
use crate::resolver::Resolver;

pub struct Handler {
    pub resolver: Resolver,
}

impl Handler {
    pub fn new() -> Handler {
        Handler {
            resolver: Resolver::new(),
        }
    }

    pub fn handle(&self, buf: &[u8], response_buf: &mut [u8]) -> usize {
        let query: DNSQuery = DNSQuery::transform_to_wire_format(buf);
        self.resolver.resolve(query, response_buf)
    }
}
