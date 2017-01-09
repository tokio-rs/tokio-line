# Tokio Line

Examples of how to implement simple pipelined and multiplexed protocols using
[tokio-proto](https://github.com/tokio-rs/tokio-proto).

## Getting Started

Clone this repo, then `cd` into `simple`. Then, run the
following command:

```
$ cargo run --example echo_client_server
```

You should see the following:

```
SERVER: "Hello"
CLIENT: "Hello"
SERVER: "Goodbye"
CLIENT: "Goodbye"
```

## More examples

This repository includes a number of examples that demonstrate how to use
[tokio-proto](https://github.com/tokio-rs/tokio-proto):

* [simple](simple/src/lib.rs) implements a simple line-based protocol with an
  [example](simple/examples/echo_client_server.rs) of how to use it.
* [multiplexed](multiplexed/src/lib.rs) implements a simple multiplexed
  protocol with an [example](multiplexed/examples/echo_client_server.rs) of how
  to use it.
* [streaming](streaming/src/lib.rs) implements a line-based protocol that is
  able to stream requests and responses with an
  [example](streaming/examples/stdout_server.rs) of how to use it.
* [handshake](simple/examples/handshake.rs) shows how to handle the handshake
  phase of a protocol, this may include SSL, authentication, etc...
* [ping_pong](simple/examples/ping_pong.rs) shows how to implement protocol
  logic at the transport layer.
* [stream_client](simple/examples/stream_client.rs) shows how to use a transport
  directly without using tokio-proto. This makes sense for protocols that aren't
  request / response oriented.

## License

Tokio is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
