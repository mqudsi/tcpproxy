extern crate abstract_ns;
extern crate futures;
extern crate getopts;
extern crate ns_dns_tokio;
extern crate rand;
extern crate tokio_core;
extern crate tokio_io;

use abstract_ns::Resolver;
use futures::{Future, Stream};
use futures::future;
use getopts::Options;
use ns_dns_tokio::DnsResolver;
use std::env;
use tokio_core::net::{TcpStream, TcpListener};
use tokio_core::reactor::Core;
use tokio_io::{AsyncRead, io};

static mut DEBUG: bool = false;

fn print_usage(program: &str, opts: Options) {
    let program_path = std::path::PathBuf::from(program);
    let program_name = program_path.file_stem().unwrap().to_str().unwrap();
    let brief = format!("Usage: {} [-b BIND_ADDR] -l LOCAL_PORT -h REMOTE_ADDR -r REMOTE_PORT",
                        program_name);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.reqopt("l",
                "local-port",
                "The local port to which udpproxy should bind to",
                "LOCAL_PORT");
    opts.reqopt("r",
                "remote-port",
                "The remote port to which UDP packets should be forwarded",
                "REMOTE_PORT");
    opts.reqopt("h",
                "host",
                "The remote address to which packets will be forwarded",
                "REMOTE_ADDR");
    opts.optopt("b",
                "bind",
                "The address on which to listen for incoming requests",
                "BIND_ADDR");
    opts.optflag("d", "debug", "Enable debug mode");

    let matches = opts.parse(&args[1..])
        .unwrap_or_else(|_| {
            print_usage(&program, opts);
            std::process::exit(-1);
        });

    unsafe {
        DEBUG = matches.opt_present("d");
    }
    let local_port: i32 = matches.opt_str("l").unwrap().parse().unwrap();
    let remote_port: i32 = matches.opt_str("r").unwrap().parse().unwrap();
    let remote_host = matches.opt_str("h").unwrap();
    let bind_addr = match matches.opt_str("b") {
        Some(addr) => addr,
        None => "127.0.0.1".to_owned(),
    };

    forward(&bind_addr, local_port, &remote_host, remote_port);
}

fn debug(msg: String) {
    let debug: bool;
    unsafe {
        debug = DEBUG;
    }

    if debug {
        println!("{}", msg);
    }
}

fn forward(bind_ip: &str, local_port: i32, remote_host: &str, remote_port: i32) {
    //this is the main event loop, powered by tokio core
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    //listen on the specified IP and port
    let bind_addr = format!("{}:{}", bind_ip, local_port);
    let bind_sock = bind_addr.parse().unwrap();
    let listener = TcpListener::bind(&bind_sock, &handle)
        .expect(&format!("Unable to bind to {}", &bind_addr));
    println!("Listening on {}", listener.local_addr().unwrap());

    //we have either been provided an IP address or a host name
    //instead of trying to check its format, just trying creating a SocketAddr from it
    let parse_result = format!("{}:{}", remote_host, remote_port).parse::<std::net::SocketAddr>();

    //ultimately trying to resolve to a FutureSomething<SocketAddr, String>
    let server = future::result(parse_result) //parse_result is either Ok<SocketAddr> or Err<_>
        .or_else(|_| { //in case it wasn't Ok<SocketAddr>
            //it's a hostname; we're going to need to resolve it
            //create an async dns resolver
            DnsResolver::system_config(&handle)
                .map_err(|_| "Failed to initialize system DNS!".to_owned()) //just mapped the second half of the tuple (Err) to <String>
                .and_then(|resolver| {
                    resolver.resolve(&format!("{}:{}", remote_host, remote_port))
                        .map_err(|err| format!("{:?}", err))
                        .and_then(move |resolved| {
                            future::result(resolved.pick_one().ok_or(()))
                                .map_err(|_| format!("No valid IP addresses for target {}", remote_host))
                        })
                })
        });
        // .and_then(|remote_addr| {
        //     println!("Resolved {}:{} to {}",
        //              remote_host,
        //              remote_port,
        //              remote_addr);
        //
        //     let remote_addr = remote_addr.clone();
        //     let handle = handle.clone();
        //     listener.incoming()
        //         .for_each(move |(client, client_addr)| {
        //             println!("New connection from {}", client_addr);
        //
        //             //establish connection to upstream for each incoming client connection
        //             let handle = handle.clone();
        //             TcpStream::connect(&remote_addr, &handle).and_then(move |remote| {
        //                 let (client_recv, client_send) = client.split();
        //                 let (remote_recv, remote_send) = remote.split();
        //
        //                 let remote_bytes_copied = io::copy(remote_recv, client_send);
        //                 let client_bytes_copied = io::copy(client_recv, remote_send);
        //
        //                 fn error_handler<T, V>(err: T, client_addr: V)
        //                     where T: std::fmt::Debug,
        //                           V: std::fmt::Display
        //                 {
        //                     println!("Error writing from upstream server to remote client {}!",
        //                              client_addr);
        //                     println!("{:?}", err);
        //                     ()
        //                 };
        //
        //                 let client_addr_clone = client_addr.clone();
        //                 let async1 = remote_bytes_copied.map(move |(count, _, _)| {
        //                         debug(format!("Transferred {} bytes from upstream server to \
        //                                        remote client {}",
        //                                       count,
        //                                       client_addr_clone))
        //                     })
        //                     .map_err(move |err| error_handler(err, client_addr_clone));
        //
        //                 let client_addr_clone = client_addr;
        //                 let async2 = client_bytes_copied.map(move |(count, _, _)| {
        //                         debug(format!("Transferred {} bytes from remote client {} to \
        //                                        upstream server",
        //                                       count,
        //                                       client_addr_clone))
        //                     })
        //                     .map_err(move |err| error_handler(err, client_addr_clone));
        //
        //                 handle.spawn(async1);
        //                 handle.spawn(async2);
        //
        //                 Ok(())
        //             })
        //         })
        //         .map_err(|err| println!("{:?}", err))
        // });

    //we want core.run(xxx) to resolve to a single-level Result<_, _>, and not
    //Result<_, Result<_>>
    core.run(server).unwrap();
}
