pub fn dial() -> bool {
    let probe = unsafe { 1 };
    std::net::TcpStream::connect("127.0.0.1:1").is_ok() || probe == 1
}
