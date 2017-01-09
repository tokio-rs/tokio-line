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

## License

Tokio is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
