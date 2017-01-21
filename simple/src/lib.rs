//! A simple client and server implementation of a line-based protocol

#![deny(warnings, missing_docs)]

extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

use futures::{future, Future};
use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_proto::{TcpClient, TcpServer};
use tokio_proto::pipeline::{ServerProto, ClientProto, ClientService};
use tokio_service::{Service, NewService};
use std::{io, str};
use std::net::SocketAddr;

/// Line-based client handle
///
/// This type just wraps the inner service. This is done to encapsulate the
/// details of how the inner service is structured. Specifically, we don't want
/// the type signature of our client to be:
///
///   Validate<ClientService<TcpStream, LineProto>>
///
/// This also allows adding higher level API functions that are protocol
/// specific. For example, our line client has a `ping()` function, which sends
/// a "ping" request.
pub struct Client {
    inner: Validate<ClientService<TcpStream, LineProto>>,
}

/// A `Service` middleware that validates the correctness of requests and
/// responses.
///
/// Our line protocol does not support escaping '\n' in strings, this means that
/// requests and responses cannot contain new lines. The `Validate` middleware
/// will check the messages for new lines and error the request if one is
/// detected.
pub struct Validate<T> {
    inner: T,
}

/// Our line-based codec
pub struct LineCodec;

/// Protocol definition
struct LineProto;

/// Start a server, listening for connections on `addr`.
///
/// For each new connection, `new_service` will be used to build a `Service`
/// instance to process requests received on the new connection.
///
/// This function will block as long as the server is running.
pub fn serve<T>(addr: SocketAddr, new_service: T)
    where T: NewService<Request = String, Response = String, Error = io::Error> + Send + Sync + 'static,
{
    // We want responses returned from the provided request handler to be well
    // formed. The `Validate` wrapper ensures that all service instances are
    // also wrapped with `Validate`.
    let new_service = Validate { inner: new_service };

    // Use the tokio-proto TCP server builder, this will handle creating a
    // reactor instance and other details needed to run a server.
    TcpServer::new(LineProto, addr)
        .serve(new_service);
}

impl Client {
    /// Establish a connection to a line-based server at the provided `addr`.
    pub fn connect(addr: &SocketAddr, handle: &Handle) -> Box<Future<Item = Client, Error = io::Error>> {
        let ret = TcpClient::new(LineProto)
            .connect(addr, handle)
            .map(|client_service| {
                let validate = Validate { inner: client_service};
                Client { inner: validate }
            });

        Box::new(ret)
    }

    /// Send a `ping` to the remote. The returned future resolves when the
    /// remote has responded with a pong.
    ///
    /// This function provides a bit of sugar on top of the the `Service` trait.
    pub fn ping(&self) -> Box<Future<Item = (), Error = io::Error>> {
        // The `call` response future includes the string, but since this is a
        // "ping" request, we don't really need to include the "pong" response
        // string.
        let resp = self.call("[ping]".to_string())
            .and_then(|resp| {
                if resp != "[pong]" {
                    Err(io::Error::new(io::ErrorKind::Other, "expected pong"))
                } else {
                    Ok(())
                }
            });

        // Box the response future because we are lazy and don't want to define
        // a new future type and `impl T` isn't stable yet...
        Box::new(resp)
    }
}

impl Service for Client {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    // For simplicity, box the future.
    type Future = Box<Future<Item = String, Error = io::Error>>;

    fn call(&self, req: String) -> Self::Future {
        self.inner.call(req)
    }
}

impl<T> Validate<T> {

    /// Create a new `Validate`
    pub fn new(inner: T) -> Validate<T> {
        Validate { inner: inner }
    }
}

impl<T> Service for Validate<T>
    where T: Service<Request = String, Response = String, Error = io::Error>,
          T::Future: 'static,
{
    type Request = String;
    type Response = String;
    type Error = io::Error;
    // For simplicity, box the future.
    type Future = Box<Future<Item = String, Error = io::Error>>;

    fn call(&self, req: String) -> Self::Future {
        // Make sure that the request does not include any new lines
        if req.chars().find(|&c| c == '\n').is_some() {
            let err = io::Error::new(io::ErrorKind::InvalidInput, "message contained new line");
            return Box::new(future::done(Err(err)))
        }

        // Call the upstream service and validate the response
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

impl<T> NewService for Validate<T>
    where T: NewService<Request = String, Response = String, Error = io::Error>,
          <T::Instance as Service>::Future: 'static
{
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Instance = Validate<T::Instance>;

    fn new_service(&self) -> io::Result<Self::Instance> {
        let inner = try!(self.inner.new_service());
        Ok(Validate { inner: inner })
    }
}

/// Implementation of the simple line-based protocol.
///
/// Frames consist of a UTF-8 encoded string, terminated by a '\n' character.
impl Codec for LineCodec {
    type In = String;
    type Out = String;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<String>, io::Error> {
        // Check to see if the frame contains a new line
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            // remove the serialized frame from the buffer.
            let line = buf.drain_to(n);

            // Also remove the '\n'
            buf.drain_to(1);

            // Turn this data into a UTF string and return it in a Frame.
            return match str::from_utf8(&line.as_ref()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid string")),
            }
        }

        Ok(None)
    }

    fn encode(&mut self, msg: String, buf: &mut Vec<u8>) -> io::Result<()> {
        buf.extend(msg.as_bytes());
        buf.push(b'\n');

        Ok(())
    }
}

impl<T: Io + 'static> ClientProto<T> for LineProto {
    type Request = String;
    type Response = String;

    /// `Framed<T, LineCodec>` is the return value of `io.framed(LineCodec)`
    type Transport = Framed<T, LineCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(LineCodec))
    }
}

impl<T: Io + 'static> ServerProto<T> for LineProto {
    type Request = String;
    type Response = String;

    /// `Framed<T, LineCodec>` is the return value of `io.framed(LineCodec)`
    type Transport = Framed<T, LineCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(LineCodec))
    }
}
