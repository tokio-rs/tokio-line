extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_line as line;
extern crate env_logger;

use futures::Future;
use tokio_core::Loop;
use tokio_proto::Service;

pub fn main() {
    env_logger::init().unwrap();

    // First thing we need is a Tokio reactor which watches all our sources (e.g. sockets,
    // channels) for readiness and then makes sure our code is actually only called when there is
    // something to do.
    let mut lp = Loop::new().unwrap();

    // To actually give over control flow to the reactor, we call reactor.spawn() later. Tokio
    // takes control of our program then, everything is asynchronous from there. To still
    // communicate with the reactor we need a handle which we can clone at will.
    let handle = lp.handle();

    // This brings up our server.
    let addr = "127.0.0.1:12345".parse().unwrap();
    line::service::serve(
        handle.clone(),
        addr,
        tokio_proto::simple_service(|msg| {
            println!("GOT: {:?}", msg);
            Ok(msg)
        }));

    // Now our client. We use the same reactor as for the server - usually though this would be
    // done in a separate program most likely on a separate machine.
    let client = line::client::connect(handle, &addr);

    // The connect call returns us a ClientHandle that allows us to use the 'Service' as a function
    // - one that returns a future that we can 'await' on.
    let resp = client.and_then(|client| {
        client.call("Hello".to_string())
    });
    println!("RESPONSE: {:?}", lp.run(resp));
}
