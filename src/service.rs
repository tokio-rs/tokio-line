use tokio_service::{Service, NewService};
use tokio_core::io::Io;
use tokio_proto::TcpServer;
use tokio_proto::pipeline::ServerProto;
use futures::{Future};
use std::io;
use std::net::SocketAddr;
use {LineTransport, new_line_transport};

/// We want to encapsulate `proto::Message`. Since the line protocol does
/// not have any streaming bodies, we can make the service be a request &
/// response of type String. `LineService` takes the service supplied to
/// `serve` and adapts it to work with the `proto::pipeline::Server`
/// requirements.
struct LineService<T> {
    inner: T,
}

struct NewLineService<T> {
    inner: T,
}

struct LineProto;

impl<T> Service for LineService<T>
    where T: Service<Request = String, Response = String, Error = io::Error>,
          T::Future: 'static,
{
    type Request = String;
    type Response = String;
    type Error = io::Error;

    // To make things easier, we are just going to box the future here, however
    // it is possible to not box the future and refer to `futures::Map`
    // directly.
    type Future = Box<Future<Item = Self::Response, Error = io::Error>>;

    fn call(&mut self, req: String) -> Self::Future {
        Box::new(self.inner.call(req)
            .and_then(|resp| {
                if resp.chars().find(|&c| c == '\n').is_some() {
                    Err(io::Error::new(io::ErrorKind::InvalidInput, "message contained new line"))
                } else {
                    Ok(resp)
                }
            }))
    }
}

impl<T: Io + 'static> ServerProto<T> for LineProto {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Transport = LineTransport<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(new_line_transport(io))
    }
}

impl<T> NewService for NewLineService<T>
    where T: NewService<Request = String, Response = String, Error = io::Error>,
          <T::Instance as Service>::Future: 'static
{
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Instance = LineService<T::Instance>;

    fn new_service(&self) -> io::Result<Self::Instance> {
        let inner = try!(self.inner.new_service());
        Ok(LineService { inner: inner })
    }
}

/// Serve a service up. Secret sauce here is 'NewService', a helper that must be able to create a
/// new 'Service' for each connection that we receive.
pub fn serve<T>(addr: SocketAddr, new_service: T)
    where T: NewService<Request = String, Response = String, Error = io::Error> + Send + Sync + 'static,
{
    let new_service = NewLineService { inner: new_service };

    TcpServer::new(LineProto, addr)
        .serve(new_service);
}
