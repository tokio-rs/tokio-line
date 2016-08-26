use tokio::io::{Readiness, Transport};
use tokio::proto::pipeline;
use std::{io, mem};

/// Line transport. This is a pretty bare implementation of a Transport that is chunked into
/// individual lines. We have a higher level version in version2: It uses higher level abstractions
/// to make the job of parsing a framed transport simpler. 
/// The job of a transport is twofold:
///
/// 1) take the bytes that arrive on our 'inner' (e.g. socket) and chunk them down into frames as
///    Transport::In.
/// 2) take the frames as Transport::Out to send out and turn them into bytes. This allows for
///    example for combining multiple frames into one TCP send.
///
/// The Service only deals in The magic here is that 'inner' must implement 'Readiness' - this allows it to play with Tokio's
/// reactor.
pub struct LowLevelLineTransport<T> {
    inner: T,
    read_buffer: Vec<u8>,
    write_buffer: io::Cursor<Vec<u8>>,
}

pub fn new_line_transport<T>(inner: T) -> LowLevelLineTransport<T> 
    where T: io::Read + io::Write + Readiness,
{
    LowLevelLineTransport {
        inner: inner,
        read_buffer: vec![],
        write_buffer: io::Cursor::new(vec![]),
    }
}

impl<T> Readiness for LowLevelLineTransport<T>
    where T: Readiness
{
    // Our transport is ready for reading whenever our 'inner'.
    fn is_readable(&self) -> bool {
        self.inner.is_readable()
    }

    // And ready for writing whenever inner is. Below we make sure that we always write everything
    // out to 'inner' whenever it is ready, so our writing buf should always be empty when 'inner'
    // is ready for writing and non-empty if it isn't.
    fn is_writable(&self) -> bool {
        let is_writable = self.write_buffer.position() == self.write_buffer.get_ref().len() as u64;

        if !is_writable {
            assert!(!self.inner.is_writable());
        }

        is_writable
    }
}

/// This defines the chunks written to our transport, i.e. the representation
/// that the `Service` deals with. In our case, the received and sent frames
/// are mostly the same (Strings with io::Error as failures), however they
/// could also be different (for example HttpRequest for In and HttpResponse
/// for Out).
pub type Frame = pipeline::Frame<String, io::Error>;

/// This is a bare-metal implementation of a Transport. We define our frames to be String when
/// reading from the wire, that is 'In' and also String when writing to the wire.
impl<T> Transport for LowLevelLineTransport<T>
    where T: io::Read + io::Write + Readiness
{
    type In = Frame;
    type Out = Frame;

    /// Read a message from the `Transport`
    fn read(&mut self) -> io::Result<Option<Frame>> {
        loop {
            // First, we check if our read buffer contains a new line - if that is the case, we
            // have one new Frame for the Service to consume. We remove the line from the input
            // buffer and this function will get called by Tokio soon again to see if there are
            // more frames available.
            if let Some(n) = self.read_buffer.iter().position(|b| *b == b'\n') {
                let tail = self.read_buffer.split_off(n+1);
                let mut line = mem::replace(&mut self.read_buffer, tail);

                // Remove the new line
                line.truncate(n);

                return String::from_utf8(line)
                    // For pipelined protocols, the message must be a tuple
                    // of the message payload to be sent to the Service and
                    // Option<Sender<T>> where T is the body chunk type.
                    //
                    // To support streaming bodies, the transport could create
                    // a channel pair, include the receiving end in the message
                    // payload and provide the sending end to the pipeline
                    // protocol dispatcher which will then proxy any body chunk
                    // frame to the Sender.
                    .map(|s| Some(pipeline::Frame::Message(s)))
                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid string"));
            }

            // There was no full line in the input buffer - let's see if anything is on our
            // 'inner'. 
            match self.inner.read_to_end(&mut self.read_buffer) {
                Ok(0) => {
                    // The other side hang up - this transport is all done.
                    // TODO(sirver): The use case of this is not entirely clear to me.
                    return Ok(Some(pipeline::Frame::Done));
                },
                Ok(_) => {
                    // Some data was read. The next round in the loop will try to parse it into a
                    // line again.
                }
                Err(e) => {
                    // This would block - i.e. there is no data on the socket. We signal Tokio that
                    // there is right now no full frame available. It will try again the next time
                    // our source signals readiness to read.
                    if e.kind() == io::ErrorKind::WouldBlock {
                        return Ok(None);
                    }

                    // Just a regular error - pass upwards for handling.
                    return Err(e)
                }
            }
        }
    }

    /// Write a message to the `Transport`. This turns the frame we get into a byte string and adds
    /// a newline. It then immediately gets flushed out to 'inner'.
    fn write(&mut self, req: Frame) -> io::Result<Option<()>> {
        match req {
            pipeline::Frame::Message(req) => {
                trace!("writing value; val={:?}", req);
                // Our write buffer can only be non-empty if our 'inner' is not ready for writes.
                // But since we signal to Tokio that our Transport is not ready when 'inner' is not
                // ready it should never try to write to us as long as our write buffer is not
                // empty.
                if self.write_buffer.position() < self.write_buffer.get_ref().len() as u64 {
                    return Err(io::Error::new(io::ErrorKind::Other, "transport has pending writes"));
                }

                let mut bytes = req.into_bytes();
                bytes.push(b'\n');

                self.write_buffer = io::Cursor::new(bytes);
                self.flush()
            }
            _ => unimplemented!(),
        }
    }

    /// Flush pending writes to the socket. This tries to write as much as possible of the data we
    /// have in the write buffer to 'inner'. Since this might block - because inner is not ready,
    /// we have to keep track of what we wrote.
    fn flush(&mut self) -> io::Result<Option<()>> {
        trace!("flushing transport");
        loop {
            // Making the borrow checker happy
            let res = {
                let buf = {
                    let pos = self.write_buffer.position() as usize;
                    let buf = &self.write_buffer.get_ref()[pos..];

                    if buf.is_empty() {
                        trace!("transport flushed");
                        return Ok(Some(()));
                    }

                    trace!("writing; remaining={:?}", buf);

                    buf
                };

                self.inner.write(buf)
            };

            match res {
                Ok(mut n) => {
                    n += self.write_buffer.position() as usize;
                    self.write_buffer.set_position(n as u64)
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        trace!("transport flush would block");
                        return Ok(None);
                    }

                    trace!("transport flush error; err={:?}", e);
                    return Err(e)
                }
            }
        }
    }
}


