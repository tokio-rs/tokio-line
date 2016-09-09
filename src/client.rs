use futures::{Async, Future};
use std::io;
use std::net::SocketAddr;
use tokio_service::Service;
use proto::pipeline;
use tokio::reactor::Handle;
use tokio::net::TcpStream;
use futures::stream::Empty;
use new_line_transport;
use std::cell::RefCell;

/// And the client handle.
pub struct Client {
    inner: pipeline::Client<String, String, Empty<(), io::Error>, io::Error>,
}

impl Service for Client {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    // Again for simplicity, we are just going to box a future
    type Future = Box<Future<Item = Self::Response, Error = io::Error>>;

    fn call(&self, req: String) -> Self::Future {
        self.inner.call(pipeline::Message::WithoutBody(req))
            .boxed()
    }

    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}

pub fn connect(handle: Handle, addr: &SocketAddr)
               -> Box<Future<Item=Client, Error=io::Error>> {
    Box::new(TcpStream::connect(addr, &handle)
        .and_then(move |tcp| {
            let tcp = RefCell::new(Some(tcp));
            let c = try!(pipeline::connect(&handle, move || {
                // Not an ideal strategy, but fixing this requires some
                // upstream changes.
                let tcp = tcp.borrow_mut().take().unwrap();
                Ok(new_line_transport(tcp))
            }));

            Ok(Client { inner: c })
        }))
}
