use futures::Future;
use std::io;
use std::net::SocketAddr;
use tokio_service::Service;
use proto::proto::pipeline;
use tokio::LoopHandle;
use tokio::io::IoFuture;
use empty::Empty;
use new_line_transport;
use std::cell::RefCell;

/// And the client handle.
pub struct Client {
    inner: pipeline::Client<String, String, Empty<(), io::Error>, io::Error>,
}

impl Service for Client {
    type Req = String;
    type Resp = String;
    type Error = io::Error;
    // Again for simplicity, we are just going to box a future
    type Fut = Box<Future<Item = Self::Resp, Error = io::Error> + Send>;

    fn call(&self, req: String) -> Self::Fut {
        self.inner.call(pipeline::Message::WithoutBody(req))
            .boxed()
    }
}

pub fn connect(loop_handle: LoopHandle, addr: &SocketAddr) -> IoFuture<Client> {
    let h2 = loop_handle.clone();
    loop_handle.tcp_connect(addr)
        .and_then(move |tcp| {
            let tcp = RefCell::new(Some(tcp));
            let c = pipeline::connect(h2, move || {
                // Not an ideal strategy, but fixing this requires some
                // upstream changes.
                let tcp = tcp.borrow_mut().take().unwrap();
                Ok(new_line_transport(tcp))
            });

            Ok(Client { inner: c })
        }).boxed()
}
