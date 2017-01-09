//! Using tokio-proto to get a request / response oriented client and server
//!
//! This example is similar to `echo_client_server`, however it also illustrates
//! how to augment a transport to support additional protocol level details.
//!
//! In this case, we are adding a keep alive feature to our line-based protocol.
//! Every so often, the client will send a "ping" message to the remote, and the
//! remote is expected to immediately respond with a pong. It doesn't really
//! make sense to expose the ping / pong at the service level because handling
//! it isn't application specific. So, we end up handling it at the transport
//! layer.

extern crate tokio_line as line;

#[macro_use]
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate service_fn;

use futures::{Future, Stream, Poll, Async};
use futures::{Sink, AsyncSink, StartSend};

use tokio_core::io::{Framed, Io};
use tokio_core::reactor::Core;

use tokio_proto::TcpServer;
use tokio_proto::pipeline::ServerProto;

use tokio_service::{Service, NewService};

use service_fn::service_fn;
use std::{io, thread};
use std::net::SocketAddr;
use std::time::Duration;

/// The PingPong will be composed of a transport and will intercept any [ping]
/// messages and immediately respond with a [pong].
struct PingPong<T> {
    // The upstream transport
    upstream: T,
    // Number of remaining pongs to send
    pongs_remaining: usize,
}

/// Implement `Stream` for our transport "middleware"
impl<T> Stream for PingPong<T>
    where T: Stream<Item = String, Error = io::Error>,
          T: Sink<SinkItem = String, SinkError = io::Error>,
{
    type Item = String;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<String>, io::Error> {
        loop {
            // Poll the upstream transport. `try_ready!` will bubble up errors
            // and Async::NotReady.
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg == "[ping]" => {
                    // Intercept [ping] messages
                    self.pongs_remaining += 1;

                    // Try flushing the pong, only bubble up errors
                    try!(self.poll_complete());
                }
                m => return Ok(Async::Ready(m)),
            }
        }
    }
}

/// Implement Sink for our transport "middleware"
impl<T> Sink for PingPong<T>
    where T: Sink<SinkItem = String, SinkError = io::Error>,
{
    type SinkItem = String;
    type SinkError = io::Error;

    fn start_send(&mut self, item: String) -> StartSend<String, io::Error> {
        // Only accept the write if there are no pending pongs
        if self.pongs_remaining > 0 {
            return Ok(AsyncSink::NotReady(item));
        }

        // If there are no pending pongs, then send the item upstream
        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        while self.pongs_remaining > 0 {
            // Try to send the pong upstream
            let res = try!(self.upstream.start_send("[pong]".to_string()));

            if !res.is_ready() {
                // The upstream is not ready to accept new items
                break;
            }

            // The pong has been sent upstream
            self.pongs_remaining -= 1;
        }

        // Call poll_complete on the upstream
        //
        // If there are remaining pongs to send, this call may create additional
        // capacity. One option could be to attempt to send the pongs again.
        // However, if a `start_send` returned NotReady, and this poll_complete
        // *did* create additional capacity in the upstream, then *our*
        // `poll_complete` will get called again shortly.
        //
        // Hopefully this makes sense... it probably doesn't, so please ask
        // questions in the Gitter channel and help me explain this better :)
        self.upstream.poll_complete()
    }
}

/// Our custom `LineProto` that will include ping / pong
struct LineProto;

/// Start a server, listening for connections on `addr`.
///
/// Similar to the `serve` function in `lib.rs`, however since we are changing
/// the protocol to include ping / pong, we will need to use the tokio-proto
/// builders directly
pub fn serve<T>(addr: SocketAddr, new_service: T)
    where T: NewService<Request = String, Response = String, Error = io::Error> + Send + Sync + 'static,
{
    // We want responses returned from the provided request handler to be well
    // formed. The `Validate` wrapper ensures that all service instances are
    // also wrapped with `Validate`.
    let new_service = line::Validate::new(new_service);

    // Use the tokio-proto TCP server builder, this will handle creating a
    // reactor instance and other details needed to run a server.
    TcpServer::new(LineProto, addr)
        .serve(new_service);
}

impl<T: Io + 'static> ServerProto<T> for LineProto {
    type Request = String;
    type Response = String;
    type Error = io::Error;

    /// `Framed<T, LineCodec>` is the return value of `io.framed(LineCodec)`
    type Transport = PingPong<Framed<T, line::LineCodec>>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(PingPong {
            upstream: io.framed(line::LineCodec),
            pongs_remaining: 0,
        })
    }
}

pub fn main() {
    let mut core = Core::new().unwrap();

    // This brings up our server.
    let addr = "127.0.0.1:12345".parse().unwrap();

    thread::spawn(move || {
        // Use our `serve` fn
        serve(
            addr,
            || {
                Ok(service_fn(|msg| {
                    println!("SERVER: {:?}", msg);
                    Ok(msg)
                }))
            });
    });

    // A bit annoying, but we need to wait for the server to connect
    thread::sleep(Duration::from_millis(100));

    let handle = core.handle();

    core.run(
        line::Client::connect(&addr, &handle)
            .and_then(|client| {
                // Start with a ping
                client.ping()
                    .and_then(move |_| {
                        println!("Pong received...");
                        client.call("Goodbye".to_string())
                    })
                    .and_then(|response| {
                        println!("CLIENT: {:?}", response);
                        Ok(())
                    })
            })
    ).unwrap();
}
