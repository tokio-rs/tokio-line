extern crate tokio;
extern crate futures;

// NOCOM(#sirver): only required for version2
extern crate bytes; 

#[macro_use]
extern crate log;

// We provide two implementations of the transport in this code walk. Both implement exactly the
// same functionality. This first one is bare metal - it does it's own buffer management to stitch
// partial messages that come from the socket together into full frames that our service can
// actually use. In production code, the second implementation which uses higher level abstractions
// would be preferable, but this one exposes the core tokio constructs better and has therefore
// higher educational value. 
pub mod transport_version1;
// pub use transport_version1::LineTransport1 as LineTransport;
// pub use transport_version1::new_line_transport;

// This is the second implementation of the transport. It uses tokio::io::Framed - which works with
// the concept of a Parser and Serializer and works with higher level abstractions from the bytes
// crate. Its implementation is much simpler and less error prone, and would be the correct choice
// in production code. 
pub mod transport_version2;
pub use transport_version2::LineTransport2 as LineTransport;
pub use transport_version2::new_line_transport;

// NOCOM(#sirver): more docu.
pub mod service;

pub mod client;
