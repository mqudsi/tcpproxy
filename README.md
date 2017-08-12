# tcpproxy
_a simple, cross-platform, multi-client TCP proxy_

`tcpproxy` is a cross-platform, multi-client TCP proxy written in rust, that is designed for those "one-time" tasks where you usually end up spending more time installing a proxy server and setting up the myriad configuration files and options than you do actually using it.

## Usage

`tcpproxy` is a command-line application. One instance of `tcpproxy` should be started for each remote endpoint you wish to proxy data to/from. All configuration is done via command-line arguments, in keeping with the spirit of this project.

```
tcpproxy [-b BIND_ADDR] -l LOCAL_PORT -h REMOTE_ADDR -r REMOTE_PORT

Options:
    -l, --local-port LOCAL_PORT
                        The local port to which tcpproxy should bind to
    -r, --remote-port REMOTE_PORT
                        The remote port to which TCP packets should be
                        forwarded
    -h, --host REMOTE_ADDR
                        The remote address to which packets will be forwarded
    -b, --bind BIND_ADDR
                        The address on which to listen for incoming requests
    -d, --debug         Enable debug mode
```

Where possible, sane defaults for arguments are provided automatically.

## Installation

`tcpproxy` is available via `crate`, the rust package manager. Installation is as follows:

    cargo install tcpproxy

Pre-complied binaries for select platforms may be available from the `tcpproxy` homepage at https://neosmart.net/tcpproxy/

## License

`tcpproxy` is open source and licensed under the terms of the MIT public license.
