extern crate getopts;
extern crate rand;

use getopts::Options;
use std::env;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const TIMEOUT: u64 = 3 * 60 * 100; //3 minutes
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

fn forward(bind_addr: &str, local_port: i32, remote_host: &str, remote_port: i32) {
    let local_addr = format!("{}:{}", bind_addr, local_port);
    let local = TcpListener::bind(&local_addr).expect(&format!("Unable to bind to {}", &local_addr));
    println!("Listening on {}", local.local_addr().unwrap());

    let remote_addr = format!("{}:{}", remote_host, remote_port);

    loop {
        let client_id;
        let client = match local.accept() {
            Ok((x, y)) => {
                client_id = format!("{:?}", y);
                println!("New connection from client {:?}", client_id);
                x
            },
            Err(e) => {
                println!("Error establishing connection to client: {:?}", e);
                continue;
            }
        };

        let remote_addr_copy = remote_addr.clone();
        thread::spawn(move|| {
            let mut timeouts : u64 = 0;
            let timed_out = Arc::new(AtomicBool::new(false));

            let local_timed_out = timed_out.clone();
            //while with UDP we had one thread to read and write from a single (upstream|client)
            //connection, with TCP a thread will read from one and write to the other.

            //this thread reads from upstream and writes to client
            let mut client_send = client.try_clone().expect("Could not clone client connection!");
            let mut upstream_recv = TcpStream::connect(&remote_addr_copy)
                .expect("Failed to open connection to remote address!");
            let mut client_recv = client;
            let mut upstream_send = upstream_recv.try_clone()
                .expect("Failed to clone client-specific connection to upstream!");
            let client_id_copy = client_id.clone();

            thread::spawn(move|| {
                let mut from_upstream = [0; 8 * 1024];
                upstream_recv.set_read_timeout(Some(Duration::from_millis(TIMEOUT + 100))).unwrap();
                client_send.set_write_timeout(Some(Duration::from_millis(TIMEOUT + 100))).unwrap();

                loop {
                    // debug("Waiting for data from upstream server".to_owned());
                    match upstream_recv.read(&mut from_upstream) {
                        Ok(0) => {
                            // continue;
                            break;
                        },
                        Ok(bytes_rcvd) => {
                            debug(format!("Received {} bytes from upstream server", bytes_rcvd));
                            let mut total_bytes_written = 0;
                            while total_bytes_written != bytes_rcvd {
                                let bytes_written = client_send.write(&from_upstream[total_bytes_written..bytes_rcvd - total_bytes_written])
                                    .expect("Failed to queue response from upstream server for forwarding!");
                                debug(format!("Wrote {} bytes to client", bytes_written));
                                total_bytes_written += bytes_written;
                            }
                            timeouts = 0; //reset timeout count
                        },
                        Err(_) => {
                            if local_timed_out.load(Ordering::Relaxed) {
                                debug(format!("Terminating forwarder thread for client {} due to timeout", client_id));
                                break;
                            }
                        }
                    };
                }
            });

            let mut from_client = [0; 8 * 1024];
            client_recv.set_read_timeout(Some(Duration::from_millis(TIMEOUT + 100))).unwrap();
            upstream_send.set_write_timeout(Some(Duration::from_millis(TIMEOUT + 100))).unwrap();
            loop {
                // debug("Waiting for data from client".to_owned());
                match client_recv.read(&mut from_client) {
                    Ok(0) => {
                        // continue;
                        break;
                    },
                    Ok(bytes_rcvd) => {
                        debug(format!("Received {} bytes from client", bytes_rcvd));
                        let mut total_bytes_written = 0;
                        while total_bytes_written != bytes_rcvd {
                            let bytes_written = upstream_send.write(&from_client[total_bytes_written..bytes_rcvd - total_bytes_written])
                                .expect("Failed to queue response from upstream server for forwarding!");
                            debug(format!("Wrote {} bytes to upstream", bytes_written));
                            total_bytes_written += bytes_written;
                        }
                        timeouts = 0; //reset timeout count
                    },
                    Err(_) => {
                        timeouts += 1;
                        if timeouts >= 10 {
                            debug(format!("Disconnecting forwarder for client {} due to timeout", client_id_copy));
                            timed_out.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                };
            }
        });
    }
}
