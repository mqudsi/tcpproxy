use getopts::Options;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::join;
use tokio::net::{TcpListener, TcpStream};

type BoxedError = Box<dyn std::error::Error + Sync + Send + 'static>;
static DEBUG: AtomicBool = AtomicBool::new(false);

fn print_usage(program: &str, opts: Options) {
    let program_path = std::path::PathBuf::from(program);
    let program_name = program_path.file_stem().unwrap().to_string_lossy();
    let brief = format!("Usage: {} REMOTE_HOST:PORT [-b BIND_ADDR] [-l LOCAL_PORT]",
                        program_name);
    print!("{}", opts.usage(&brief));
}

#[tokio::main]
async fn main() -> Result<(), BoxedError> {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("b",
                "bind",
                "The address on which to listen for incoming requests, defaulting to localhost",
                "BIND_ADDR");
    opts.optopt("l",
                "local-port",
                "The local port to which tcpproxy should bind to, randomly chosen otherwise",
                "LOCAL_PORT");
    opts.optflag("d", "debug", "Enable debug mode");

    let matches = match opts.parse(&args[1..]) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("{}", e);
            print_usage(&program, opts);
            std::process::exit(-1);
        }
    };
    let remote = match matches.free.len() {
        1 => matches.free[0].as_str(),
        _ => {
            print_usage(&program, opts);
            std::process::exit(-1);
        },
    };

    if !remote.contains(':') {
        eprintln!("A remote port is required (REMOTE_ADDR:PORT)");
        std::process::exit(-1);
    }

    DEBUG.store(matches.opt_present("d"), Ordering::Relaxed);
    // let local_port: i32 = matches.opt_str("l").unwrap_or("0".to_string()).parse()?;
    let local_port: i32 = matches.opt_str("l").map(|s| s.parse()).unwrap_or(Ok(0))?;
    let bind_addr = match matches.opt_str("b") {
        Some(addr) => addr,
        None => "127.0.0.1".to_owned(),
    };

    forward(&bind_addr, local_port, remote).await
}

async fn forward(bind_ip: &str, local_port: i32, remote: &str) -> Result<(), BoxedError> {
    // Listen on the specified IP and port
    let bind_addr = if !bind_ip.starts_with('[') && bind_ip.contains(':') {
        // Correctly format for IPv6 usage
        format!("[{}]:{}", bind_ip, local_port)
    } else {
        format!("{}:{}", bind_ip, local_port)
    };
    let bind_sock = bind_addr.parse::<std::net::SocketAddr>().expect("Failed to parse bind address");
    let listener = TcpListener::bind(&bind_sock).await?;
    println!("Listening on {}", listener.local_addr().unwrap());

    // We have either been provided an IP address or a host name.
    // Instead of trying to check its format, just trying creating a SocketAddr from it.
    // let parse_result = remote.parse::<SocketAddr>();
    let remote = std::sync::Arc::new(remote.to_string());

    loop {
        let remote = remote.clone();
        let (mut client, client_addr) = listener.accept().await?;

        tokio::spawn(async move {
                println!("New connection from {}", client_addr);

                // Establish connection to upstream for each incoming client connection
                let mut remote = TcpStream::connect(remote.as_str()).await?;
                let (mut client_recv, mut client_send) = client.split();
                let (mut remote_recv, mut remote_send) = remote.split();

                // This version of the join! macro does not require that the futures are fused and
                // pinned prior to passing to join.
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
