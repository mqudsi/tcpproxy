use futures::FutureExt;
use getopts::Options;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

type BoxedError = Box<dyn std::error::Error + Sync + Send + 'static>;
static DEBUG: AtomicBool = AtomicBool::new(false);
const BUF_SIZE: usize = 1024;

fn print_usage(out: &mut dyn std::io::Write, program: &str, opts: Options) {
    let program_path = std::path::PathBuf::from(program);
    let program_name = program_path.file_stem().unwrap().to_string_lossy();
    let brief = format!(
        "Usage: {} REMOTE_HOST:PORT [-b BIND_ADDR] [-l LOCAL_PORT]",
        program_name
    );
    _ = write!(out, "{}", opts.usage(&brief));
}

#[tokio::main]
async fn main() -> Result<(), BoxedError> {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt(
        "b",
        "bind",
        "The address on which to listen for incoming requests, defaulting to localhost",
        "BIND_ADDR",
    );
    opts.optopt(
        "l",
        "local-port",
        "The local port to which tcpproxy should bind to, randomly chosen otherwise",
        "LOCAL_PORT",
    );
    opts.optflag("d", "debug", "Enable debug mode");

    let matches = match opts.parse(&args[1..]) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("{}", e);
            print_usage(&mut std::io::stderr().lock(), &program, opts);
            std::process::exit(-1);
        }
    };
    let remote = match matches.free.len() {
        1 => matches.free[0].clone(),
        _ => {
            print_usage(&mut std::io::stderr().lock(), &program, opts);
            std::process::exit(-1);
        }
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

async fn forward(bind_ip: &str, local_port: i32, remote: String) -> Result<(), BoxedError> {
    // Listen on the specified IP and port
    let bind_addr = if !bind_ip.starts_with('[') && bind_ip.contains(':') {
        // Correctly format for IPv6 usage
        format!("[{}]:{}", bind_ip, local_port)
    } else {
        format!("{}:{}", bind_ip, local_port)
    };
    let bind_sock = bind_addr
        .parse::<std::net::SocketAddr>()
        .expect("Failed to parse bind address");
    let listener = TcpListener::bind(&bind_sock).await?;
    println!("Listening on {}", listener.local_addr().unwrap());

    // `remote` should be either the host name or ip address, with the port appended.
    // It doesn't get tested/validated until we get our first connection, though!

    // We leak `remote` instead of wrapping it in an Arc to share it with future tasks since
    // `remote` is going to live for the lifetime of the server in all cases.
    // (This reduces MESI/MOESI cache traffic between CPU cores.)
    let remote: &str = Box::leak(remote.into_boxed_str());

    // Two instances of this function are spawned for each half of the connection: client-to-server,
    // server-to-client. We can't use tokio::io::copy() instead (no matter how convenient it might
    // be) because it doesn't give us a way to correlate the lifetimes of the two tcp read/write
    // loops: even after the client disconnects, tokio would keep the upstream connection to the
    // server alive until the connection's max client idle timeout is reached.
    async fn copy_with_abort<R, W>(
        read: &mut R,
        write: &mut W,
        mut abort: broadcast::Receiver<()>,
    ) -> tokio::io::Result<usize>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut copied = 0;
        let mut buf = [0u8; BUF_SIZE];
        loop {
            let bytes_read;
            tokio::select! {
                biased;

                result = read.read(&mut buf) => {
                    use std::io::ErrorKind::{ConnectionReset, ConnectionAborted};
                    bytes_read = result.or_else(|e| match e.kind() {
                        // Consider these to be part of the proxy life, not errors
                        ConnectionReset | ConnectionAborted => Ok(0),
                        _ => Err(e)
                    })?;
                },
                _ = abort.recv() => {
                    break;
                }
            }

            if bytes_read == 0 {
                break;
            }

            // While we ignore some read errors above, any error writing data we've already read to
            // the other side is always treated as exceptional.
            write.write_all(&buf[0..bytes_read]).await?;
            copied += bytes_read;
        }

        Ok(copied)
    }

    loop {
        let (mut client, client_addr) = listener.accept().await?;

        tokio::spawn(async move {
            println!("New connection from {}", client_addr);

            // Establish connection to upstream for each incoming client connection
            let mut remote = match TcpStream::connect(remote).await {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Error establishing upstream connection: {e}");
                    return;
                }
            };
            let (mut client_read, mut client_write) = client.split();
            let (mut remote_read, mut remote_write) = remote.split();

            let (cancel, _) = broadcast::channel::<()>(1);
            let (remote_copied, client_copied) = tokio::join! {
                copy_with_abort(&mut remote_read, &mut client_write, cancel.subscribe())
                    .then(|r| { let _ = cancel.send(()); async { r } }),
                copy_with_abort(&mut client_read, &mut remote_write, cancel.subscribe())
                    .then(|r| { let _ = cancel.send(()); async { r } }),
            };

            match client_copied {
                Ok(count) => {
                    if DEBUG.load(Ordering::Relaxed) {
                        eprintln!(
                            "Transferred {} bytes from proxy client {} to upstream server",
                            count, client_addr
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Error writing bytes from proxy client {} to upstream server",
                        client_addr
                    );
                    eprintln!("{}", err);
                }
            };

            match remote_copied {
                Ok(count) => {
                    if DEBUG.load(Ordering::Relaxed) {
                        eprintln!(
                            "Transferred {} bytes from upstream server to proxy client {}",
                            count, client_addr
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Error writing from upstream server to proxy client {}!",
                        client_addr
                    );
                    eprintln!("{}", err);
                }
            };

            ()
        });
    }
}
