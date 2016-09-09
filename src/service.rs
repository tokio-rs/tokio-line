use proto::{server, NewService};
use proto::pipeline;
use tokio_service::Service;
use tokio::reactor::Handle;
use futures::{Async, Future};
use futures::stream::Empty;
use std::io;
use std::net::SocketAddr;
use new_line_transport;

/// We want to encapsulate `pipeline::Message`. Since the line protocol does
/// not have any streaming bodies, we can make the service be a request &
/// response of type String. `LineService` takes the service supplied to
/// `serve` and adapts it to work with the `proto::pipeline::Server`
/// requirements.
struct LineService<T> {
    inner: T,
}

impl<T> Service for LineService<T>
    where T: Service<Request = String, Response = String, Error = io::Error>,
          T::Future: 'static,
{
    type Request = String;
    type Response = pipeline::Message<String, Empty<(), io::Error>>;
    type Error = io::Error;

    // To make things easier, we are just going to box the future here, however
    // it is possible to not box the future and refer to `futures::Map`
    // directly.
    type Future = Box<Future<Item = Self::Response, Error = io::Error>>;

    fn call(&self, req: String) -> Self::Future {
        Box::new(self.inner.call(req)
            .map(pipeline::Message::WithoutBody))
    }

    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}

/// Serve a service up. Secret sauce here is 'NewService', a helper that must be able to create a
/// new 'Service' for each connection that we receive.
pub fn serve<T>(handle: &Handle,  addr: SocketAddr, new_service: T)
                -> io::Result<()>
    where T: NewService<Request = String, Response = String, Error = io::Error> + Send + 'static,
{
    try!(server::listen(handle, addr, move |stream| {
        // Initialize the pipeline dispatch with the service and the line
        // transport
        let service = LineService { inner: try!(new_service.new_service()) };
        pipeline::Server::new(service, new_line_transport(stream))
    }));
    Ok(())
}
