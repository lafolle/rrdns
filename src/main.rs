// Baby steps
#[macro_use]
extern crate slice_as_array;

mod business;
mod handler;
mod resolver;
mod server;

fn main() {
    // let addr = "127.0.0.1:8888";
    // let server = DNSServer::new(&addr);
    // server.listen();

    server::run();
}
