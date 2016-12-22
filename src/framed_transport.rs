use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use std::{io, str};

pub struct LineCodec;

impl Codec for LineCodec {
    type In = String;
    type Out = String;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<String>, io::Error> {
        // If our buffer contains a newline...
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            // remove this line and the newline from the buffer.
            let line = buf.drain_to(n);
            buf.drain_to(1); // Also remove the '\n'.

            // Turn this data into a UTF string and return it in a Frame.
            return match str::from_utf8(line.as_ref()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid string")),
            }
        }

        Ok(None)
    }

    fn encode(&mut self, msg: String, buf: &mut Vec<u8>) -> io::Result<()> {
        for byte in msg.as_bytes() {
            buf.push(*byte);
        }

        buf.push(b'\n');
        Ok(())
    }
}

pub type FramedLineTransport<T> = Framed<T, LineCodec>;

pub fn new_line_transport<T>(inner: T) -> FramedLineTransport<T>
    where T: Io,
{
    inner.framed(LineCodec)
}
