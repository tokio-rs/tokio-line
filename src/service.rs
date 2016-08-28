use std::io;
use std::net::SocketAddr;

use futures::{Future, BoxFuture};
use futures::stream::Receiver;
use tokio_core::LoopHandle;
use tokio_proto::proto::pipeline;
use tokio_proto::{server, Service, NewService};

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
    where T: Service<Req = String, Resp = String, Error = io::Error>,
{
    type Req = String;
    type Resp = pipeline::Message<String, Receiver<(), io::Error>>;
    type Error = io::Error;

    // To make things easier, we are just going to box the future here, however
    // it is possible to not box the future and refer to `futures::Map`
    // directly.
    type Fut = BoxFuture<Self::Resp, io::Error>;

    fn call(&self, req: String) -> Self::Fut {
        self.inner.call(req)
            .map(pipeline::Message::WithoutBody)
            .boxed()
    }
}

/// Serve a service up. Secret sauce here is 'NewService', a helper that must be able to create a
/// new 'Service' for each connection that we receive.
pub fn serve<T>(handle: LoopHandle, addr: SocketAddr, new_service: T)
    where T: NewService<Req = String, Resp = String, Error = io::Error> + Send + 'static {
    server::listen(handle, addr, move |stream| {
        // Initialize the pipeline dispatch with the service and the line
        // transport
        let service = LineService { inner: try!(new_service.new_service()) };
        pipeline::Server::new(service, new_line_transport(stream))
    }).forget();
}
