extern crate futures;
extern crate tokio;
extern crate tokio_line as line;
extern crate env_logger;

use futures::Future;
use tokio::Service;

pub fn main() {
    env_logger::init().unwrap();

    let addr = "127.0.0.1:12345".parse().unwrap();

    line::Server::new()
        .bind(addr)
        .serve(tokio::simple_service(|msg| {
            println!("GOT: {:?}", msg);
            Ok(msg)
        }))
        .unwrap();


    let client = line::Client::new()
        .connect(&addr)
        .unwrap();

    let resp = client.call("Hello".to_string());
    println!("RESPONSE: {:?}", await(resp));

    drop(client);
}

// Why this isn't in futures-rs, I do not know...
fn await<T: Future>(f: T) -> Result<T::Item, T::Error> {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();

    f.then(move |res| {
        tx.send(res).unwrap();
        Ok::<(), ()>(())
    }).forget();

    rx.recv().unwrap()
}
