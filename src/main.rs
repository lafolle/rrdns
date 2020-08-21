// Baby steps
use crate::business::models::DNSQueryResponse;
use crate::handler::Handler;
use clap::{App, Arg, ArgMatches};
use error::FetchError;
use hyper::header::HeaderValue;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Method, Request, Response, Server, StatusCode};
use lazy_static::lazy_static;
use log::{debug, error, info};
use prometheus::{
    self, exponential_buckets, register_histogram_vec, register_int_counter, Encoder, HistogramVec,
    IntCounter, TextEncoder,
};
use serde_json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::ops::Deref;
use std::panic;
use std::sync::Arc;
use std::time::Instant;
use tokio;
use tokio::sync::mpsc;

mod business;
mod error;
mod handler;
mod reactor;
mod resolver;
mod server;

lazy_static! {
    static ref RRDNS_QUERY_COUNTER: IntCounter =
        register_int_counter!("rrdns_query_count", "number of queries").unwrap();
    static ref RRDNS_RESOLUTION_FAILURE: IntCounter =
        register_int_counter!("rrdns_resolution_failure", "Number of failed resolutions.").unwrap();
    static ref RRDNS_RESOLUTION_DURATION: HistogramVec = register_histogram_vec!(
        "rrdns_resolution_duration",
        "Histogram of latencies of resolution",
        &["resolveit"],
        exponential_buckets(0.005, 2.0, 10).unwrap()
    )
    .unwrap();
    static ref RRDNS_QUERY_SIZE: HistogramVec = register_histogram_vec!(
        "rrdns_query_size",
        "Histogram of size of queries",
        &["querysize"],
        exponential_buckets(0.005, 2.0, 10).unwrap()
    )
    .unwrap();
    static ref RRDNS_QUERY_RESPONSE_SIZE: HistogramVec = register_histogram_vec!(
        "rrdns_query_response_size",
        "Histogram of size of query responses",
        &["queryresponsesize"],
        exponential_buckets(0.005, 2.0, 10).unwrap()
    )
    .unwrap();
}

fn init<'a>() -> ArgMatches<'a> {
    // Initialize the logger.
    env_logger::init();

    // Set panic
    panic::set_hook(Box::new(|panic_info| {
        let (filename, line) = panic_info
            .location()
            .map(|loc| (loc.file(), loc.line()))
            .unwrap_or(("<unknown>", 0));

        let cause = panic_info
            .payload()
            .downcast_ref::<String>()
            .map(String::deref);
        let cause = cause.unwrap_or_else(|| {
            panic_info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| *s)
                .unwrap_or("<cause unknown>")
        });

        error!("PANIC occured at {}:{} {}", filename, line, cause);
    }));

    // Parse CLI args.
    let matches = App::new("rrdns")
        .version("0.0")
        .author("lafolle")
        .about("Recursive DNS resolver in Rust")
        .arg(
            Arg::with_name("listen")
                // .short("l")
                .long("listen")
                .takes_value(true)
                .help("Server will listen for DNS queries on this address"),
        )
        .arg(
            Arg::with_name("listen_debug")
                // .short("ld")
                .long("listen_debug")
                .takes_value(true)
                .help("Debug server will listen for debug queries on this address"),
        )
        .arg(
            Arg::with_name("listen_metrics")
                // .short("lm")
                .long("listen_metrics")
                .takes_value(true)
                .help("Prometheus metrics will be exposed on this address"),
        )
        .get_matches();

    matches
}

async fn metrics_service(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    info!("Prometheus metrics requested.");

    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    let nf = prometheus::gather();
    encoder.encode(&nf, &mut buffer).unwrap();
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buffer))
        .unwrap())
}

struct RRDNSServer {
    listen_addr: SocketAddr,
    listen_debug_addr: SocketAddr,
    listen_metrics: SocketAddr,
}

#[tokio::main]
async fn main() {
    let matches = init();
    let listen_addr = matches
        .value_of("listen")
        .unwrap_or("127.0.0.1:8888")
        .to_string();
    let listen_debug_addr = matches
        .value_of("listen_debug")
        .unwrap_or("127.0.0.1:7777")
        .to_string();
    let listen_metrics_addr = matches
        .value_of("listen_metrics")
        .unwrap_or("127.0.0.1:9999")
        .to_string();

    let handler = Arc::new(Handler::new());

    tokio::spawn(async move {
        let prometheus_exposition_addr = listen_metrics_addr.parse::<SocketAddr>().unwrap();
        let make_svc =
            make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(metrics_service)) });
        let server = Server::bind(&prometheus_exposition_addr).serve(make_svc);
        info!(
            "Metrics (prometheus) server binded to {}",
            listen_metrics_addr
        );
        if let Err(e) = server.await {
            error!("prometheus: error={}", e);
        }
    });

    let debug_handler = handler.clone();
    tokio::spawn(async move {
        let debug_addr = listen_debug_addr.parse::<SocketAddr>().unwrap();

        let make_svc = make_service_fn(|_socket: &AddrStream| {
            let debug_handler = debug_handler.clone();

            async move {
                Ok::<_, Error>(service_fn(move |req| {
                    let cache = debug_handler.clone_cache();

                    let jsoned_cache = serde_json::to_string(&cache).unwrap();
                    let mut response = Response::new(Body::empty());

                    async move {
                        match (req.method(), req.uri().path()) {
                            (&Method::GET, "/debug/cache") => {
                                *response.body_mut() = Body::from(jsoned_cache);
                                response.headers_mut().insert(
                                    "Content-type",
                                    HeaderValue::from_static("application/json"),
                                );
                            }
                            _ => *response.status_mut() = StatusCode::NOT_FOUND,
                        };

                        Ok::<_, Error>(response)
                    }
                }))
            }
        });

        let server = Server::bind(&debug_addr).serve(make_svc);

        info!("Debug server binded to {}", debug_addr);

        if let Err(e) = server.await {
            error!("server error: {}", e);
        }
    });

    let addr = listen_addr.parse::<SocketAddr>().unwrap();
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    info!("DNS resolver binded to address {}", listen_addr);
    let (mut socket_rx, mut socket_tx) = socket.split();
    let (response_tx, mut response_rx) =
        mpsc::channel::<(Result<DNSQueryResponse, FetchError>, SocketAddr, Instant)>(5);

    // Read DNS queries from socket.
    let read_handler = tokio::spawn(async move {
        let mut buf = [0; 1024];
        while let Ok((bytes_read_count, peer)) = socket_rx.recv_from(&mut buf).await {
            debug!("bytes read count: {}", bytes_read_count);
            RRDNS_QUERY_SIZE
                .with_label_values(&["querysize"])
                .observe(bytes_read_count as f64);
            let mut dst_buf = vec![0; bytes_read_count];
            dst_buf[..].copy_from_slice(&mut buf[..bytes_read_count]);
            tokio::spawn(process(dst_buf, peer, handler.clone(), response_tx.clone()));
        }
    });

    // Write DNS responses to socket.
    let write_handler = tokio::spawn(async move {
        loop {
            let (response_result, peer, start_instant) = response_rx.recv().await.unwrap();
            match response_result {
                Ok(response) | Err(FetchError::QueryError(response)) => {
                    let raw_response = response.serialize();
                    let written_bytes = socket_tx.send_to(&raw_response, &peer).await.unwrap();
                    let latency = start_instant.elapsed();
                    RRDNS_RESOLUTION_DURATION
                        .with_label_values(&["resolveit"])
                        .observe(latency.as_secs_f64());
                    RRDNS_QUERY_RESPONSE_SIZE
                        .with_label_values(&["queryresponsesize"])
                        .observe(raw_response.len() as f64);
                    debug!(
                        "{} written_bytes={} latency={:?}",
                        response.query.header.id, written_bytes, latency
                    );
                }
                Err(FetchError::NetworkError(err)) => {
                    // What to do?
                    RRDNS_RESOLUTION_FAILURE.inc();
                    error!("ISE::NetworkError={}", err);
                }
                Err(FetchError::InfiniteRecursionError(err)) => {
                    // Not sending any response back for now.
                    RRDNS_RESOLUTION_FAILURE.inc();
                    error!("terminal err={}", err)
                }
                Err(FetchError::NoIPError(err)) => {
                    RRDNS_RESOLUTION_FAILURE.inc();
                    error!("no ip error={}", err);
                }
            }
        }
    });

    read_handler.await.unwrap();
    write_handler.await.unwrap();
    // debug_server_handler.await.unwrap();
}

async fn process(
    buf: Vec<u8>, // propagated
    peer: SocketAddr,
    handler: Arc<Handler>,
    mut response_tx: mpsc::Sender<(Result<DNSQueryResponse, FetchError>, SocketAddr, Instant)>,
) {
    let start = Instant::now();
    RRDNS_QUERY_COUNTER.inc();
    let response_result = handler.handle(&buf).await;
    response_tx
        .send((response_result, peer, start))
        .await
        .unwrap();
}
