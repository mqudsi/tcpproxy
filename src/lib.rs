use std::sync::atomic::{AtomicBool, Ordering};

use futures::FutureExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};

pub type BoxedError = Box<dyn std::error::Error + Sync + Send + 'static>;
pub static DEBUG: AtomicBool = AtomicBool::new(false);
const BUF_SIZE: usize = 1024;

pub async fn forward(bind_ip: &str, local_port: i32, remote: &str) -> Result<(), BoxedError> {
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

    // We have either been provided an IP address or a host name.
    let remote = std::sync::Arc::new(remote.to_string());

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
                    bytes_read = result?;
                },
                _ = abort.recv() => {
                    break;
                }
            }

            if bytes_read == 0 {
                break;
            }

            write.write_all(&buf[0..bytes_read]).await?;
            copied += bytes_read;
        }

        Ok(copied)
    }

    loop {
        let remote = remote.clone();
        let (mut client, client_addr) = listener.accept().await?;

        tokio::spawn(async move {
            println!("New connection from {}", client_addr);

            // Establish connection to upstream for each incoming client connection
            let mut remote = TcpStream::connect(remote.as_str()).await?;
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
                            "Transferred {} bytes from remote client {} to upstream server",
                            count, client_addr
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Error writing bytes from remote client {} to upstream server",
                        client_addr
                    );
                    eprintln!("{}", err);
                }
            };

            match remote_copied {
                Ok(count) => {
                    if DEBUG.load(Ordering::Relaxed) {
                        eprintln!(
                            "Transferred {} bytes from upstream server to remote client {}",
                            count, client_addr
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Error writing from upstream server to remote client {}!",
                        client_addr
                    );
                    eprintln!("{}", err);
                }
            };

            let r: Result<(), BoxedError> = Ok(());
            r
        });
    }
}
