//! An echo server that times out
//!
//! The server can be run by executing:
//!
//! ```
//! cargo run --example echo_server_with_timeout
//! ```
//!
//! Then connect to it using telnet.

extern crate futures;
extern crate tokio_core as tokio;
extern crate tokio_line as line;
extern crate tokio_service as service;
extern crate tokio_timer as timer;
extern crate tokio_middleware as middleware;
extern crate service_fn;
extern crate rand;
extern crate env_logger;

use futures::Future;
use service_fn::service_fn;
use timer::Timer;
use rand::Rng;
use std::io;
use std::time::Duration;

pub fn main() {
    env_logger::init().unwrap();

    let timer = Timer::default();

    // The address to bind the listener socket to
    let addr = "127.0.0.1:12345".parse().unwrap();

    // Function that constructs a service
    let new_service = move || {
        // The service to run
        let service = {
            let timer = timer.clone();

            service_fn(move |msg| {
                // Sleep for a random duration that could be greater than the
                // alloted timeout
                let mut rng = rand::thread_rng();
                let ms = rng.next_u64() % 500;

                timer.sleep(Duration::from_millis(ms))
                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "lol wat"))
                    .and_then(|_| Ok(msg))
            })
        };

        // Decorate the service with the timeout middleware
        let service = middleware::Timeout::new(
            service, timer.clone(),
            Duration::from_millis(200));

        // Decorate the service with the Log middleware
        let service = middleware::Log::new(service);

        Ok(service)
    };

    println!("Echo server running on {}", addr);

    // Start the server
    line::service::serve(addr, new_service);
}
