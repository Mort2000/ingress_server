#![forbid(unsafe_code)]

use std::net::TcpStream;

pub fn dial(addr: &str) -> bool {
    TcpStream::connect(addr).is_ok()
}
