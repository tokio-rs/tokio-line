# Tokio Line Proto

A very basic protocol built on top of Tokio. The line protocol is UTF-8
strings such that each message is terminated with a new line. The
library shows how to implement a client and server on using Tokio and
the example shows how to consume the client and server.

To run the examples, use `cargo run --example`:

```
$ cargo run --example echo_client_server
     Running `target/debug/examples/echo_client_server`
GOT: "Hello"
RESPONSE: Ok("Hello")
```

## License

Tokio is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
