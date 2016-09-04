extern crate futures;
extern crate tokio_core as tokio;
extern crate tokio_line as line;
extern crate tokio_service as service;
extern crate env_logger;

use futures::Future;
use tokio::{Loop, LoopHandle};
use service::Service;
use std::time::Duration;
use std::thread;

pub fn main() {
    env_logger::init().unwrap();

    run(|handle| {
        // This brings up our server.
        let addr = "127.0.0.1:12345".parse().unwrap();

        let server = line::service::serve(
            handle.clone(),
            addr,
            service::simple_service(|msg| {
                println!("GOT: {:?}", msg);
                Ok(msg)
            }));

        // Wait a bit for the server to start
        // TODO: Do something better
        thread::sleep(Duration::from_millis(200));

        // Now our client. We use the same reactor as for the server - usually though this would be
        // done in a separate program most likely on a separate machine.
        let client = line::client::connect(handle.clone(), &addr).wait().unwrap();

        // The connect call returns us a ClientHandle that allows us to use the 'Service' as a function
        // - one that returns a future that we can 'await' on.
        let resp = client.call("Hello".to_string());
        println!("RESPONSE: {:?}", resp.wait());

        drop(client);
        drop(server);
    });
}

// TODO: Figure out how to move this into the core Tokio libs
fn run<F: FnOnce(LoopHandle)>(f: F) {
    use std::sync::mpsc;

    let (tx1, rx1) = futures::oneshot();
    let (tx2, rx2) = mpsc::channel();

    let t = thread::spawn(move || {
        let mut l = Loop::new().unwrap();
        tx2.send(l.handle()).unwrap();

        l.run(rx1)
    });

    f(rx2.recv().unwrap());

    tx1.complete(());
    t.join().unwrap().unwrap();
}
