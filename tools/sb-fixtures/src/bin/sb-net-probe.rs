//! sb-net-probe <host:port>: attempts one TCP connect. Landlock
//! (ABI >= 4, TCP deny) must refuse it inside the sandbox.
//! Prints `CONNECT=ok` or `CONNECT=denied kind=...`; exits 0 on
//! success, 1 on denial.

#![forbid(unsafe_code)]

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

fn main() {
    let target = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9".into());
    let mut parts = target.splitn(2, ':');
    let host = parts.next().unwrap_or("127.0.0.1").to_string();
    let port: u16 = parts.next().unwrap_or("9").parse().unwrap_or(9);
    let addrs: Vec<_> = (host.as_str(), port)
        .to_socket_addrs()
        .map(|it| it.collect())
        .unwrap_or_default();
    let mut last = String::from("no-address");
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
            Ok(_) => {
                println!("CONNECT=ok");
                std::process::exit(0);
            }
            Err(error) => last = format!("{:?}", error.kind()),
        }
    }
    println!("CONNECT=denied kind={last}");
    std::process::exit(1);
}
