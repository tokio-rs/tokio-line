use std::cell::RefCell;
use std::io;
use std::net::SocketAddr;

use futures::{Future, BoxFuture};
use futures::stream::Receiver;
use new_line_transport;
use tokio_core::LoopHandle;
use tokio_core::io::IoFuture;
use tokio_proto::Service;
use tokio_proto::proto::pipeline;

/// And the client handle.
pub struct Client {
    inner: pipeline::Client<String, String, Receiver<(), io::Error>, io::Error>,
}

impl Service for Client {
    type Req = String;
    type Resp = String;
    type Error = io::Error;
    // Again for simplicity, we are just going to box a future
    type Fut = BoxFuture<Self::Resp, io::Error>;

    fn call(&self, req: String) -> Self::Fut {
        self.inner.call(pipeline::Message::WithoutBody(req))
            .boxed()
    }
}

pub fn connect(handle: LoopHandle, addr: &SocketAddr) -> IoFuture<Client> {
    handle.clone().tcp_connect(addr).map(|tcp| {
        let tcp = RefCell::new(Some(tcp));
        let client = pipeline::connect(handle, move || {
            Ok(new_line_transport(tcp.borrow_mut().take().unwrap()))
        });

        Client { inner: client }
    }).boxed()
}
