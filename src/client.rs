use futures::Future;
use std::io;
use std::net::SocketAddr;
use tokio::Service;
use tokio::proto::pipeline;
use tokio::reactor::ReactorHandle;
use tokio::tcp::TcpStream;
use tokio::util::future::Empty;
use new_line_transport;

/// And the client handle.
pub struct Client {
    inner: pipeline::Client<String, String, Empty<(), io::Error>, io::Error>,
}

impl Service for Client {
    type Req = String;
    type Resp = String;
    type Error = io::Error;
    // Again for simplicity, we are just going to box a future
    type Fut = Box<Future<Item = Self::Resp, Error = io::Error>>;

    fn call(&self, req: String) -> Self::Fut {
        self.inner.call(pipeline::Message::WithoutBody(req))
            .boxed()
    }
}

pub fn connect(reactor: ReactorHandle, addr: &SocketAddr) -> io::Result<Client> {
    let addr = addr.clone();
    let client = pipeline::connect(&reactor, move || {
        let stream = try!(TcpStream::connect(&addr));
        Ok(new_line_transport(stream))
    });

    Ok(Client { inner: client })
}
