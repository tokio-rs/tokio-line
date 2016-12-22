use futures::{self, Future};
use std::io;
use std::net::SocketAddr;
use tokio_service::Service;
use tokio_core::io::Io;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpStream;
use tokio_proto::TcpClient;
use tokio_proto::pipeline::{ClientProto, ClientService};
use {new_line_transport, LineTransport};

/// And the client handle.
pub struct Client {
    inner: ClientService<TcpStream, LineProto>,
}

struct LineProto;

impl Service for Client {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    // Again for simplicity, we are just going to box a future
    type Future = Box<Future<Item = Self::Response, Error = io::Error>>;

    fn call(&mut self, req: String) -> Self::Future {
        // Make sure that the request does not include any new lines
        if req.chars().find(|&c| c == '\n').is_some() {
            let err = io::Error::new(io::ErrorKind::InvalidInput, "message contained new line");
            return Box::new(futures::done(Err(err)))
        }

        self.inner.call(req)
            .boxed()
    }
}

impl<T: Io + 'static> ClientProto<T> for LineProto {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Transport = LineTransport<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(new_line_transport(io))
    }
}

pub fn connect(addr: &SocketAddr, handle: &Handle) -> Box<Future<Item = Client, Error = io::Error>> {
    let ret = TcpClient::new(LineProto)
        .connect(addr, handle)
        .map(|c| Client { inner: c });

    Box::new(ret)
}
