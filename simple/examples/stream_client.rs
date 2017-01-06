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

pub fn server() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let remote_addr = "127.0.0.1:14566".parse().unwrap();

    let listener = TcpListener::bind(&remote_addr, &handle).unwrap();

    // Accept all incoming sockets
    let server = listener.incoming().for_each(move |(socket, _)| {
        let transport = socket.framed(LineCodec);

        let process_connection = transport.for_each(|line| {
            println!("GOT: {}", line);
            Ok(())
        })
        .map_err(|_| ());

        handle.spawn(process_connection);

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
            let transport = socket.framed(LineCodec);

            let lines_to_send: Vec<Result<String, io::Error>> = vec![
                Ok("Hello world".to_string()),
                Ok("This is another message".to_string()),
                Ok("Not much else to say".to_string()),
            ];

            transport.send_all(stream::iter(lines_to_send))
        });

    core.run(work).unwrap();

    // Wait a bit to make sure that the server had time to receive the lines and
    // print them to STDOUT
    thread::sleep(Duration::from_millis(100));
}
