use futures::{Async, AsyncSink, Poll, Stream, Sink, StartSend};
use tokio_core::io::Io;
use std::{io, mem};

/// Line transport. This is a pretty bare implementation of a Transport that is chunked into
/// individual lines. We have a higher level version in framed_transport.rs: It uses higher level abstractions
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
    where T: Io,
{
    LowLevelLineTransport {
        inner: inner,
        read_buffer: vec![],
        write_buffer: io::Cursor::new(vec![]),
    }
}

/// This is a bare-metal implementation of a Transport. We define our frames to be String when
/// reading from the wire, that is 'In' and also String when writing to the wire.
impl<T> Stream for LowLevelLineTransport<T>
    where T: Io
{
    type Item = String;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<String>, io::Error> {
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
                    .map(|s| Async::Ready(Some(s)))
                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid string"));
            }

            // There was no full line in the input buffer - let's see if anything is on our
            // 'inner'.
            match self.inner.read_to_end(&mut self.read_buffer) {
                Ok(0) => {
                    // The other side hang up - this transport is all done.
                    // TODO(sirver): The use case of this is not entirely clear to me.
                    return Ok(Async::Ready(None));
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
                        return Ok(Async::NotReady);
                    }

                    // Just a regular error - pass upwards for handling.
                    return Err(e)
                }
            }
        }
    }
}

impl<T> Sink for LowLevelLineTransport<T>
    where T: Io
{
    type SinkItem = String;
    type SinkError = io::Error;

    /// Write a message to the `Transport`. This turns the frame we get into a byte string and adds
    /// a newline. It then immediately gets flushed out to 'inner'.
    fn start_send(&mut self, req: String) -> StartSend<String, io::Error> {
        trace!("writing value; val={:?}", req);

        // Our write buffer can only be non-empty if our 'inner' is not ready for writes.
        if self.write_buffer.position() < self.write_buffer.get_ref().len() as u64 {
            return Ok(AsyncSink::NotReady(req));
        }

        let mut bytes = req.into_bytes();
        bytes.push(b'\n');

        self.write_buffer = io::Cursor::new(bytes);
        Ok(AsyncSink::Ready)
    }

    /// Flush pending writes to the socket. This tries to write as much as possible of the data we
    /// have in the write buffer to 'inner'. Since this might block - because inner is not ready,
    /// we have to keep track of what we wrote.
    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        trace!("flushing transport");

        loop {
            // Making the borrow checker happy
            let res = {
                let buf = {
                    let pos = self.write_buffer.position() as usize;
                    let buf = &self.write_buffer.get_ref()[pos..];

                    if buf.is_empty() {
                        trace!("transport flushed");
                        return Ok(Async::Ready(()));
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
                        return Ok(Async::NotReady);
                    }

                    trace!("transport flush error; err={:?}", e);
                    return Err(e)
                }
            }
        }
    }
}
