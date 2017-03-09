//! A simple client and server implementation fo a multiplexed, line-based
//! protocol

#![deny(warnings, missing_docs)]

extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate bytes;

use futures::{future, Future};

use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Encoder, Decoder, Framed};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_proto::{TcpClient, TcpServer};
use tokio_proto::multiplex::{ServerProto, ClientProto, ClientService};
use tokio_service::{Service, NewService};

use bytes::{BytesMut, Buf, BufMut, BigEndian};

use std::{io, str};
use std::net::SocketAddr;

/// Multiplexed line-based client handle
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
struct Validate<T> {
    inner: T,
}

/// Our multiplexed line-based codec
struct LineCodec;

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
    /// Establish a connection to a multiplexed line-based server at the
    /// provided `addr`.
    pub fn connect(addr: &SocketAddr, handle: &Handle) -> Box<Future<Item = Client, Error = io::Error>> {
        let ret = TcpClient::new(LineProto)
            .connect(addr, handle)
            .map(|client_service| {
                let validate = Validate { inner: client_service};
                Client { inner: validate }
            });

        Box::new(ret)
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

/// Implementation of the multiplexed line-based protocol.
///
/// Frames begin with a 4 byte header, consisting of the numeric request ID
/// encoded in network order, followed by the frame payload encoded as a UTF-8
/// string and terminated with a '\n' character:
///
/// # An example frame:
///
/// +-- request id --+------- frame payload --------+
/// |                |                              |
/// |   \x00000001   | This is the frame payload \n |
/// |                |                              |
/// +----------------+------------------------------+
///
impl Decoder for LineCodec {
    type Item = (u64, String);
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<(u64, String)>, io::Error> {
        // At least 5 bytes are required for a frame: 4 byte head + one byte
        // '\n'
        if buf.len() < 5 {
            return Ok(None);
        }

        // Check to see if the frame contains a new line, skipping the first 4
        // bytes which is the request ID
        if let Some(n) = buf.as_ref()[4..].iter().position(|b| *b == b'\n') {
            // remove the serialized frame from the buffer.
            let line = buf.split_to(n + 4);

            // Also remove the '\n'
            buf.split_to(1);

            // Deserialize the request ID
            let request_id = io::Cursor::new(&line[0..4]).get_u32::<BigEndian>() as u64;

            // Turn this data into a UTF string and return it in a Frame.
            return match str::from_utf8(&line.as_ref()[4..]) {
                Ok(s) => Ok(Some((request_id, s.to_string()))),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid string")),
            }
        }

        Ok(None)
    }
}

impl Encoder for LineCodec {
    type Item = (u64, String);
    type Error = io::Error;

    fn encode(&mut self, msg: (u64, String), buf: &mut BytesMut) -> io::Result<()> {
        // Reserve enough space for the frame
        let len = 4 + buf.len() + 1;
        buf.reserve(len);

        let (request_id, msg) = msg;

        buf.put_u32::<BigEndian>(request_id as u32);
        buf.put_slice(msg.as_bytes());
        buf.put_u8(b'\n');

        Ok(())
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> ClientProto<T> for LineProto {
    type Request = String;
    type Response = String;
    type RequestId = u64;

    /// `Framed<T, LineCodec>` is the return value of `io.framed(LineCodec)`
    type Transport = Framed<T, LineCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(LineCodec))
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for LineProto {
    type Request = String;
    type Response = String;
    type RequestId = u64;

    /// `Framed<T, LineCodec>` is the return value of `io.framed(LineCodec)`
    type Transport = Framed<T, LineCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(LineCodec))
    }
}
