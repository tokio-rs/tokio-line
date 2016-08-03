extern crate tokio;
extern crate mio;

#[macro_use]
extern crate log;

use tokio::{server, NewService};
use tokio::io::{Readiness, Transport};
use tokio::proto::pipeline;
use tokio::reactor::{Reactor, ReactorHandle};
use std::{io, mem};
use std::net::SocketAddr;

/// Line transport
pub struct Line<T> {
    inner: T,
    rd: Vec<u8>,
    wr: io::Cursor<Vec<u8>>,
}

pub struct Server {
    reactor: Option<ReactorHandle>,
    addr: Option<SocketAddr>
}

pub struct Client {
    reactor: Option<ReactorHandle>,
}

impl Server {
    pub fn new() -> Server {
        Server {
            reactor: None,
            addr: None,
        }
    }

    pub fn bind(mut self, addr: SocketAddr) -> Self {
        self.addr = Some(addr);
        self
    }

    pub fn serve<T>(self, new_service: T) -> io::Result<()>
        where T: NewService<Req = String, Resp = String, Error = io::Error> + Send + 'static,
    {
        let reactor = match self.reactor {
            Some(r) => r,
            None => {
                let reactor = try!(Reactor::default());
                let handle = reactor.handle();
                reactor.spawn();
                handle
            },
        };

        let addr = self.addr.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());

        try!(server::listen(&reactor, addr, move |stream| {
            // Initialize the pipeline dispatch with the service and the line
            // transport
            let service = try!(new_service.new_service());
            pipeline::Server::new(service, Line::new(stream))
        }));

        Ok(())
    }
}

pub type ClientHandle = pipeline::ClientHandle<String, String, io::Error>;

impl Client {
    pub fn new() -> Client {
        Client {
            reactor: None,
        }
    }

    pub fn connect(self, addr: &SocketAddr) -> io::Result<ClientHandle> {
        let reactor = match self.reactor {
            Some(r) => r,
            None => {
                let reactor = try!(Reactor::default());
                let handle = reactor.handle();
                reactor.spawn();
                handle
            },
        };

        let addr = addr.clone();

        // Connect the client
        Ok(pipeline::connect(&reactor, addr, |stream| Ok(Line::new(stream))))
    }
}

impl<T> Line<T>
    where T: io::Read + io::Write + Readiness,
{
    pub fn new(inner: T) -> Line<T> {
        Line {
            inner: inner,
            rd: vec![],
            wr: io::Cursor::new(vec![]),
        }
    }
}

pub type Frame = pipeline::Frame<String, io::Error>;

impl<T> Transport for Line<T>
    where T: io::Read + io::Write + Readiness
{
    type In = Frame;
    type Out = Frame;

    /// Read a message from the `Transport`
    fn read(&mut self) -> io::Result<Option<Frame>> {
        loop {
            if let Some(n) = self.rd.iter().position(|b| *b == b'\n') {
                let tail = self.rd.split_off(n+1);
                let mut line = mem::replace(&mut self.rd, tail);

                // Remove the new line
                line.truncate(n);

                return String::from_utf8(line)
                    .map(|s| Some(pipeline::Frame::Message(s)))
                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid string"));
            }

            match self.inner.read_to_end(&mut self.rd) {
                Ok(0) => return Ok(Some(pipeline::Frame::Done)),
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        return Ok(None);
                    }

                    return Err(e)
                }
            }
        }
    }

    /// Write a message to the `Transport`
    fn write(&mut self, req: Frame) -> io::Result<Option<()>> {
        match req {
            pipeline::Frame::Message(req) => {
                trace!("writing value; val={:?}", req);
                if self.wr.position() < self.wr.get_ref().len() as u64 {
                    return Err(io::Error::new(io::ErrorKind::Other, "transport has pending writes"));
                }

                let mut bytes = req.into_bytes();
                bytes.push(b'\n');

                self.wr = io::Cursor::new(bytes);
                self.flush()
            }
            _ => unimplemented!(),
        }
    }

    /// Flush pending writes to the socket
    fn flush(&mut self) -> io::Result<Option<()>> {
        trace!("flushing transport");
        loop {
            // Making the borrow checker happy
            let res = {
                let buf = {
                    let pos = self.wr.position() as usize;
                    let buf = &self.wr.get_ref()[pos..];

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
                    n += self.wr.position() as usize;
                    self.wr.set_position(n as u64)
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

impl<T> Readiness for Line<T>
    where T: Readiness
{
    fn is_readable(&self) -> bool {
        self.inner.is_readable()
    }

    fn is_writable(&self) -> bool {
        let is_writable = self.wr.position() == self.wr.get_ref().len() as u64;

        if !is_writable {
            assert!(!self.inner.is_writable());
        }

        is_writable
    }
}
