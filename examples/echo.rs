extern crate futures;
extern crate tokio;
extern crate tokio_line as line;
extern crate env_logger;

use futures::Future;
use tokio::reactor::{Reactor};
use tokio::Service;

pub fn main() {
    env_logger::init().unwrap();

    // First thing we need is a Tokio reactor which watches all our sources (e.g. sockets,
    // channels) for readiness and then makes sure our code is actually only called when there is
    // something to do.
    let reactor = Reactor::default().unwrap();

    // To actually give over control flow to the reactor, we call reactor.spawn() later. Tokio
    // takes control of our program then, everything is asynchronous from there. To still
    // communicate with the reactor we need a handle which we can clone at will.
    let handle = reactor.handle();
    reactor.spawn();

    // This brings up our server.
    let addr = "127.0.0.1:12345".parse().unwrap();
    line::service::serve(
        handle.clone(),
        addr,
        tokio::simple_service(|msg| {
            println!("GOT: {:?}", msg);
            Ok(msg)
        }))
        .unwrap();

    // Now our client. We use the same reactor as for the server - usually though this would be
    // done in a separate program most likely on a separate machine.
    let client = line::client::connect(handle, &addr).unwrap();

    // The connect call returns us a ClientHandle that allows us to use the 'Service' as a function
    // - one that returns a future that we can 'await' on.
    let resp = client.call("Hello".to_string());
    println!("RESPONSE: {:?}", await(resp));

    drop(client);
}

// Blocks the execution of the current thread until the future is available. Why this isn't in
// futures-rs, I do not know...
fn await<T: Future>(f: T) -> Result<T::Item, T::Error> {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();

    f.then(move |res| {
        tx.send(res).unwrap();
        Ok::<(), ()>(())
    }).forget();

    rx.recv().unwrap()
}
