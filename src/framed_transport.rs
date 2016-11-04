use tokio::io::Io;
use tokio::easy::{EasyFramed, EasyBuf, Parse, Serialize};
use futures::{Async, Poll};
use std::{io, str};

pub struct Parser;

impl Parse for Parser {
    type Out = String;

    fn parse(&mut self, buf: &mut EasyBuf) -> Poll<Self::Out, io::Error> {
        // If our buffer contains a newline...
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            // remove this line and the newline from the buffer.
            let line = buf.drain_to(n);
            buf.drain_to(1); // Also remove the '\n'.

            // Turn this data into a UTF string and return it in a Frame.
            return match str::from_utf8(line.as_ref()) {
                Ok(s) => Ok(Async::Ready(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid string")),
            }
        }

        Ok(Async::NotReady)
    }

    fn done(&mut self, buf: &mut EasyBuf) -> io::Result<Self::Out> {
        assert!(buf.as_ref().is_empty());
        // Ok(None)
        unimplemented!();
    }
}

pub struct Serializer;

impl Serialize for Serializer {
    type In = String;

    fn serialize(&mut self, frame: String, buf: &mut Vec<u8>) {
        for byte in frame.as_bytes() {
            buf.push(*byte);
        }

        buf.push(b'\n');
    }
}

pub type FramedLineTransport<T> = EasyFramed<T, Parser, Serializer>;

pub fn new_line_transport<T>(inner: T) -> FramedLineTransport<T>
    where T: Io,
{
    EasyFramed::new(inner, Parser, Serializer)
}
