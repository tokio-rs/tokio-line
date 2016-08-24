use bytes::{Buf, BlockBuf, MutBuf};
use std::io;
use std::str;
use tokio::io::Readiness;
use tokio::io::{Parse, Serialize, Framed};
use tokio::proto::pipeline;
use transport_version1::Frame;

pub struct Parser;

impl Parse for Parser {
    type Out = Frame;

    fn parse(&mut self, buf: &mut BlockBuf) -> Option<Frame> {
        // Make sure the data is continuous in memory.
        if !buf.is_compact() {
            buf.compact();
        }

        if let Some(n) = buf.bytes().unwrap().iter().position(|b| *b == b'\n') {
            let line = buf.shift(n);
            buf.shift(1); // Also remove the '\n'.

            return match str::from_utf8(line.buf().bytes()) {
                Ok(s) => Some(pipeline::Frame::Message(s.to_string())),
                Err(_) => Some(pipeline::Frame::Error(
                        io::Error::new(io::ErrorKind::Other, "invalid string"))),
            }
        }
        None
    }
}

pub struct Serializer;

impl Serialize for Serializer {
    type In = Frame;

    fn serialize(&mut self, frame: Frame, buf: &mut BlockBuf) {
        match frame {
            pipeline::Frame::Message(text) => {
                buf.write_slice(&text.as_bytes());
                buf.write_slice(&['\n' as u8]);
            }
            _ => unimplemented!(),
        }
    }
}

pub type LineTransport2<T> = Framed<T, Parser, Serializer>;

pub fn new_line_transport<T>(inner: T) -> LineTransport2<T>
    where T: io::Read + io::Write + Readiness
{
  Framed::new(inner,
              Parser,
              Serializer,
              BlockBuf::default(),
              BlockBuf::default())
}
