//! Using a transport directly
//!
//! This example illustrates a use case where the protocol isn't request /
//! response oriented. In this case, the connection is established, and "log"
//! entries are streamed to the remote.
//!
//! Given that the use case is not request / response oriented, it doesn't make
//! sense to use `tokio-proto`. Instead, we use the transport directly.

extern crate tokio_line;

extern crate futures;
extern crate tokio_core;

use tokio_line::LineCodec;

use futures::{stream, Future, Stream, Sink};

use tokio_core::io::Io;
use tokio_core::net::{TcpStream, TcpListener};
use tokio_core::reactor::Core;

use std::{io, thread};
use std::time::Duration;

/// Run the server. The server will simply listen for new connections, receive
/// strings, and write them to STDOUT.
///
/// The function will block until the server is shutdown.
pub fn server() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let remote_addr = "127.0.0.1:14566".parse().unwrap();

    let listener = TcpListener::bind(&remote_addr, &handle).unwrap();

    // Accept all incoming sockets
    let server = listener.incoming().for_each(move |(socket, _)| {
        // Use the `Io::framed` helper to get a transport from a socket. The
        // `LineCodec` handles encoding / decoding frames.
        let transport = socket.framed(LineCodec);

        // The transport is a `Stream<Item = String>`. So we can now operate at
        // at the frame level. For each received line, write the string to
        // STDOUT.
        //
        // The return value of `for_each` is a future that completes once
        // `transport` is done yielding new lines. This happens when the
        // underlying socket closes.
        let process_connection = transport.for_each(|line| {
            println!("GOT: {}", line);
            Ok(())
        });

        // Spawn a new task dedicated to processing the connection
        handle.spawn(process_connection.map_err(|_| ()));

        Ok(())
    });

    // Open listener
    core.run(server).unwrap();
}

pub fn main() {
    // Run the server in a dedicated thread
    thread::spawn(|| server());

    // Wait a moment for the server to start...
    thread::sleep(Duration::from_millis(100));

    // Connect to the remote and write some lines
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let remote_addr = "127.0.0.1:14566".parse().unwrap();

    let work = TcpStream::connect(&remote_addr, &handle)
        .and_then(|socket| {
            // Once the socket has been established, use the `framed` helper to
            // create a transport.
            let transport = socket.framed(LineCodec);

            // We're just going to send a few "log" messages to the remote
            let lines_to_send: Vec<Result<String, io::Error>> = vec![
                Ok("Hello world".to_string()),
                Ok("This is another message".to_string()),
                Ok("Not much else to say".to_string()),
            ];

            // Send all the messages to the remote. The strings will be encoded by
            // the `Codec`. `send_all` returns a future that completes once
            // everything has been sent.
            transport.send_all(stream::iter(lines_to_send))
        });

    core.run(work).unwrap();

    // Wait a bit to make sure that the server had time to receive the lines and
    // print them to STDOUT
    thread::sleep(Duration::from_millis(100));
}
