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
                        defaulting to localhost
    -l, --local-port LOCAL_PORT
                        The local port to which tcpproxy should bind to,
                        randomly chosen otherwise
    -d, --debug         Enable debug mode w/ connection logging
    -h, --help          Print usage info and exit
    -V, --version       Print version info and exit
```

Where possible, sane defaults for arguments are provided automatically.

## Installation

`tcpproxy` is available via `cargo`, the rust package manager. Installation is as follows:

    cargo install tcpproxy

Pre-complied binaries for select platforms may be available from the `tcpproxy` homepage at https://neosmart.net/tcpproxy/

## Project Status

Depending on which language ecosystem you are coming from, this project may appear to be "unmaintained." Do not be fooled by a lack of updates for some length of time - this project is regularly updated **when needed** to fix bugs, improve code quality, use more modern rust coding patterns and conventions, and update dependencies. This project is *not*, however, updated for the sake of updating and is currently, in the humble opinion of its author, fairly feature-complete. The intention was always to provide a minimalistic (but still useful!) tcp proxy that can be quickly fired-up from the command line and put to good use. It is not intended to become comprehensive of any and all peripheral features and attempts to bundle "everything and the kitchen sink" will be respectfully but firmly declined.

## Contributing

Pull requests are welcome, but for any major undertakings, please do open an issue first to make sure we're all on the same page!

## License and Authorship

`tcpproxy` is developed and maintained by Mahmoud Al-Qudsi of NeoSmart Technologies. `tcpproxy` is open source and licensed under the terms of the MIT public license, made available to the general public without warranty in the hopes that it may prove both edifying and useful.
