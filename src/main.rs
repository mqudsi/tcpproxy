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
    let addr = future::result(format!("{}:{}", remote_host, remote_port).parse::<std::net::SocketAddr>())
        .map(|result| future::result(Ok(result))) //I'm pretty sure this is wrong. We only want a single level.
        .or_else(|_| {
            //it's a hostname; we're going to need to resolve it.
            future::result(DnsResolver::system_config(&handle))
                .map(|resolver| {
                    resolver.resolve(&format!("{}:{}", remote_host, remote_port))
                        .map(move |resolved| {
                            resolved.pick_one()
                                .expect(&format!("No valid IP addresses for target {}", remote_host))
                        })
                })
        });

    //we want core.run(xxx) to resolve to a single-level Result<_, _>, and not
    //Result<_, Result<_>>
    core.run(addr).unwrap();
}
