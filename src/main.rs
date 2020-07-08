// Baby steps

#[macro_use]
extern crate slice_as_array;

mod business;
mod handler;
mod resolver;
mod server;

fn main() {
    server::run();
}
