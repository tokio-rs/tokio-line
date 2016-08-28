extern crate bytes;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;

#[macro_use]
extern crate log;

// We provide two implementations of the transport in this code walk. Both implement exactly the
// same functionality. This first one is bare metal - it does it's own buffer management to stitch
// partial messages that come from the socket together into full frames that our service can
// actually use. In production code, the second implementation which uses higher level abstractions
// would be preferable, but this one exposes the core tokio constructs better and has therefore
// higher educational value.
pub mod low_level_transport;
// pub use low_level_transport::LowLevelLineTransport as LineTransport;
// pub use low_level_transport::new_line_transport;

// This is the second implementation of the transport. It uses tokio::io::Framed - which works with
// the concept of a Parser and Serializer and works with higher level abstractions from the bytes
// crate. Its implementation is much simpler and less error prone, and would be the correct choice
// in production code.
pub mod framed_transport;
pub use framed_transport::FramedLineTransport as LineTransport;
pub use framed_transport::new_line_transport;

// Contains the definition of the service that is used both by client and server. It also contains
// the function showing how to serve a service up.
pub mod service;

// Contains the client part - connecting and calling a remote service.
pub mod client;
