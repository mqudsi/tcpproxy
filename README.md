# tcpproxy
_a simple, cross-platform, multi-client TCP proxy_

`tcpproxy` is a cross-platform, multi-client TCP proxy written in rust, that is designed for those "one-time" tasks where you usually end up spending more time installing a proxy server and setting up the myriad configuration files and options than you do actually using it.

`tcpproxy` is completely asynchronous and built on top of the `tokio` async runtime. It was written to serve as an example of how bi-directional async networking code using rust futures and an async framework would look and is intentionally kept easy to understand. The code is updated regularly to take advantage of new tokio features and best practices (if/when they change).

## Usage

`tcpproxy` is a command-line application. One instance of `tcpproxy` should be started for each remote endpoint you wish to proxy data to/from. All configuration is done via command-line arguments, in keeping with the spirit of this project.

```
tcpproxy REMOTE_HOST:PORT [-b BIND_ADDR] [-l LOCAL_PORT]

Options:
    -b, --bind BIND_ADDR
                        The address on which to listen for incoming requests,
                        defaulting to localhost.
    -l, --local-port LOCAL_PORT
                        The local port to which tcpproxy should bind to
                        listening for requests, randomly chosen otherwise.
    -d, --debug         Enables debug mode w/ connection logging.
```

Where possible, sane defaults for arguments are provided automatically.

## Installation

`tcpproxy` is available via `crate`, the rust package manager. Installation is as follows:

    cargo install tcpproxy

Pre-complied binaries for select platforms may be available from the `tcpproxy` homepage at https://neosmart.net/tcpproxy/

## License

`tcpproxy` is open source and licensed under the terms of the MIT public license.
