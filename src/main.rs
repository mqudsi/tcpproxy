use getopts::Options;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::join;
use tokio::net::{TcpListener, TcpStream};

type BoxedError = Box<dyn std::error::Error + Sync + Send + 'static>;
static DEBUG: AtomicBool = AtomicBool::new(false);

fn print_usage(program: &str, opts: Options) {
    let program_path = std::path::PathBuf::from(program);
    let program_name = program_path.file_stem().unwrap().to_str().unwrap();
    let brief = format!("Usage: {} [-b BIND_ADDR] -l LOCAL_PORT -h REMOTE_ADDR -r REMOTE_PORT",
                        program_name);
    print!("{}", opts.usage(&brief));
}

#[tokio::main]
async fn main() -> Result<(), BoxedError> {
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

    let matches = match opts.parse(&args[1..]) {
        Ok(opts) => opts,
        Err(_) => {
            print_usage(&program, opts);
            std::process::exit(-1);
        }
    };

    DEBUG.store(matches.opt_present("d"), Ordering::Relaxed);
    let local_port: i32 = matches.opt_str("l").unwrap().parse()?;
    let remote_port: i32 = matches.opt_str("r").unwrap().parse()?;
    let remote_host = matches.opt_str("h").unwrap();
    let bind_addr = match matches.opt_str("b") {
        Some(addr) => addr,
        None => "127.0.0.1".to_owned(),
    };

    forward(&bind_addr, local_port, &remote_host, remote_port).await
}

async fn forward(bind_ip: &str, local_port: i32, remote_host: &str, remote_port: i32) -> Result<(), BoxedError> {
    // Listen on the specified IP and port
    let bind_addr = if bind_ip.contains(':') {
        format!("[{}]:{}", bind_ip, local_port)
    } else {
        format!("{}:{}", bind_ip, local_port)
    };
    let bind_sock = bind_addr.parse::<std::net::SocketAddr>().expect("Failed to parse bind address");
    let listener = TcpListener::bind(&bind_sock).await?;
    println!("Listening on {}", listener.local_addr().unwrap());

    // We have either been provided an IP address or a host name.
    // Instead of trying to check its format, just trying creating a SocketAddr from it.
    let parse_result = format!("{}:{}", remote_host, remote_port).parse::<std::net::SocketAddr>();
    let remote_addr = match parse_result {
        Ok(s) => s,
        Err(_) => {
            // It's a hostname; we're going to need to resolve it.
            // Create an async dns resolver

            use trust_dns_resolver::TokioAsyncResolver;
            use trust_dns_resolver::config::*;

            let resolver = TokioAsyncResolver::tokio(
                ResolverConfig::default(),
                ResolverOpts::default())
            .await.expect("Failed to create DNS resolver");

            let resolutions = resolver.lookup_ip(remote_host).await.expect("Failed to resolve server IP address!");
            let remote_addr = resolutions.iter().nth(1).expect("Failed to resolve server IP address!");
            println!("Resolved {} to {}", remote_host, remote_addr);
            format!("{}:{}", remote_addr, remote_port).parse()?
        },
    };

    loop {
        let (mut client, client_addr) = listener.accept().await?;

        tokio::spawn(async move {
                println!("New connection from {}", client_addr);

                // Establish connection to upstream for each incoming client connection
                let mut remote = TcpStream::connect(&remote_addr).await?;
                let (mut client_recv, mut client_send) = client.split();
                let (mut remote_recv, mut remote_send) = remote.split();

                let (remote_bytes_copied, client_bytes_copied) = join!(
                    tokio::io::copy(&mut remote_recv, &mut client_send),
                    tokio::io::copy(&mut client_recv, &mut remote_send),
                );

                match remote_bytes_copied {
                    Ok(count) => {
                        if DEBUG.load(Ordering::Relaxed) {
                            eprintln!("Transferred {} bytes from remote client {} to upstream server",
                                              count, client_addr);
                        }

                    }
                    Err(err) => {
                        eprintln!("Error writing from remote client {} to upstream server!",
                                 client_addr);
                        eprintln!("{:?}", err);
                    }
                };

                match client_bytes_copied {
                    Ok(count) => {
                        if DEBUG.load(Ordering::Relaxed) {
                            eprintln!("Transferred {} bytes from upstream server to remote client {}",
                                count, client_addr);
                        }
                    }
                    Err(err) => {
                        eprintln!("Error writing bytes from upstream server to remote client {}",
                            client_addr);
                        eprintln!("{:?}", err);
                    }
                };

                let r: Result<(), BoxedError> = Ok(());
                r
        });
    }
}
