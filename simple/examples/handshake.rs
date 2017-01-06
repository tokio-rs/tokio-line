//! Using tokio-proto to get a request / response oriented client and server
//!
//! This example is similar to `echo_client_server`, however it also illustrates
//! how to implement a connection handshake before starting to accept requests.
//!
//! In this case, when a client connects to a server, it has to send the
//! following line: `You ready?`. Once the server is ready to accept requests,
//! it responds with: `Bring it!`. If the server wants to reject the client for
//! some reason, it responds with: `No! Go away!`. The client is then expected
//! to close the socket.
//!
//! To do this, we need to implement a `ClientLineProto` and a `ServerLineProto`
//! that handle the handshakes on the client and server side respectively.

extern crate tokio_line as line;

#[macro_use]
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate service_fn;

use futures::future;
use futures::{Future, Stream, Sink};

use tokio_core::io::{Framed, Io};
use tokio_core::reactor::Core;

use tokio_proto::{TcpClient, TcpServer};
use tokio_proto::pipeline::{ClientProto, ServerProto};

use tokio_service::{Service, NewService};

use service_fn::service_fn;
use std::{io, thread};
use std::net::SocketAddr;
use std::time::Duration;

struct ClientLineProto;
struct ServerLineProto;

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
    TcpServer::new(ServerLineProto, addr)
        .serve(new_service);
}

impl<T: Io + 'static> ServerProto<T> for ServerLineProto {
    type Request = String;
    type Response = String;
    type Error = io::Error;

    /// `Framed<T, LineCodec>` is the return value of `io.framed(LineCodec)`
    type Transport = Framed<T, line::LineCodec>;
    type BindTransport = Box<Future<Item = Self::Transport, Error = io::Error>>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        // Construct the line-based transport
        let transport = io.framed(line::LineCodec);

        // The handshake requires that the client sends `You ready?`, so wait to
        // receive that line. If anything else is sent, error out the connection
        let handshake = transport.into_future()
            // If the transport errors out, we don't care about the transport
            // anymore, so just keep the error
            .map_err(|(e, _)| e)
            .and_then(|(line, transport)| {
                // A line has been received, check to see if it is the handshake
                match line {
                    Some(ref msg) if msg == "You ready?" => {
                        println!("SERVER: received client handshake");
                        // Send back the acknowledgement
                        Box::new(transport.send("Bring it!".to_string())) as Self::BindTransport
                    }
                    _ => {
                        // The client sent an unexpected handshake, error out
                        // the connection
                        println!("SERVER: client handshake INVALID");
                        let err = io::Error::new(io::ErrorKind::Other, "invalid handshake");
                        Box::new(future::err(err)) as Self::BindTransport
                    }
                }
            });

        Box::new(handshake)
    }
}

impl<T: Io + 'static> ClientProto<T> for ClientLineProto {
    type Request = String;
    type Response = String;
    type Error = io::Error;

    /// `Framed<T, LineCodec>` is the return value of `io.framed(LineCodec)`
    type Transport = Framed<T, line::LineCodec>;
    type BindTransport = Box<Future<Item = Self::Transport, Error = io::Error>>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        // Construct the line-based transport
        let transport = io.framed(line::LineCodec);

        // Send the handshake frame to the server.
        let handshake = transport.send("You ready?".to_string())
            // Wait for a response from the server, if the transport errors out,
            // we don't care about the transport handle anymore, just the error
            .and_then(|transport| transport.into_future().map_err(|(e, _)| e))
            .and_then(|(line, transport)| {
                // The server sent back a line, check to see if it is the
                // expected handshake line.
                match line {
                    Some(ref msg) if msg == "Bring it!" => {
                        println!("CLIENT: received server handshake");
                        Ok(transport)
                    }
                    Some(ref msg) if msg == "No! Go away!" => {
                        // At this point, the server is at capacity. There are a
                        // few things that we could do. Set a backoff timer and
                        // try again in a bit. Or we could try a different
                        // remote server. However, we're just going to error out
                        // the connection.

                        println!("CLIENT: server is at capacity");
                        let err = io::Error::new(io::ErrorKind::Other, "server at capacity");
                        Err(err)
                    }
                    _ => {
                        println!("CLIENT: server handshake INVALID");
                        let err = io::Error::new(io::ErrorKind::Other, "invalid handshake");
                        Err(err)
                    }
                }
            });

        Box::new(handshake)
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

    let client = TcpClient::new(ClientLineProto)
        .connect(&addr, &handle)
        .map(|client_service| line::Validate::new(client_service));

    core.run(
        client
            .and_then(|mut client| {
                client.call("Goodbye".to_string())
                    .and_then(|response| {
                        println!("CLIENT: {:?}", response);
                        Ok(())
                    })
            })
    ).unwrap();
}
