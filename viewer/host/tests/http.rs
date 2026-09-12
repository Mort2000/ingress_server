use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Fixture {
    root: PathBuf,
    child: Option<Child>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "viewer-http-{}",
            arena_runner::tournament::exec_external::execution_id().unwrap()
        ));
        fs::create_dir_all(root.join("runs")).unwrap();
        fs::create_dir(root.join("web")).unwrap();
        fs::write(root.join("web/index.html"), "<title>Viewer fixture</title>").unwrap();
        Self { root, child: None }
    }
    fn start(&mut self) -> (String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_arena-viewer"))
            .args(["serve", "--runs"])
            .arg(self.root.join("runs"))
            .arg("--assets")
            .arg(self.root.join("web"))
            .args(["--listen", "127.0.0.1:0", "--poll-ms", "100"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut ready = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut ready)
            .unwrap();
        self.child = Some(child);
        let (addr, token) = ready
            .trim()
            .strip_prefix("http://")
            .expect("ready URL")
            .split_once("/#token=")
            .unwrap();
        (addr.into(), token.into())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn get(addr: &str, path: &str, token: &str) -> Vec<u8> {
    request(
        addr,
        &format!(
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
        ),
    )
}
fn request(addr: &str, text: &str) -> Vec<u8> {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    s.write_all(text.as_bytes()).unwrap();
    let mut data = Vec::new();
    s.read_to_end(&mut data).unwrap();
    data
}
fn body(bytes: &[u8]) -> &[u8] {
    &bytes[bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4..]
}

#[test]
fn serves_same_origin_page_and_discovers_a_plan_created_after_startup() {
    let mut f = Fixture::new();
    let (addr, token) = f.start();
    let html = get(&addr, "/", "");
    assert!(html.starts_with(b"HTTP/1.1 200"));
    assert_eq!(body(&html), b"<title>Viewer fixture</title>");
    let initial = get(&addr, "/api/events", &token);
    assert!(initial.starts_with(b"HTTP/1.1 200"));
    assert_eq!(
        arena_runner::tournament::archive::parse_json(std::str::from_utf8(body(&initial)).unwrap())
            .unwrap()
            .get("events")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let runner = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/arena-runner");
    let bot = runner.with_file_name("iteration-client");
    let output = Command::new(&runner)
        .args(["plan", "--out"])
        .arg(f.root.join("runs/new-event"))
        .arg("--client")
        .arg(&bot)
        .arg("--client")
        .arg(&bot)
        .args([
            "--rounds",
            "1",
            "--fast",
            "--name",
            "Alpha",
            "--name",
            "Beta",
            "--bot-version",
            "old",
            "--bot-version",
            "new",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let id = loop {
        let response = get(&addr, "/api/events", &token);
        let json = arena_runner::tournament::archive::parse_json(
            std::str::from_utf8(body(&response)).unwrap(),
        )
        .unwrap();
        let events = json.get("events").unwrap().as_array().unwrap();
        if !events.is_empty()
            && events[0].get("error") == Some(&arena_runner::tournament::archive::Json::Null)
        {
            assert_eq!(events[0].get("total").unwrap().as_int(), Some(2));
            break events[0].get("id").unwrap().as_str().unwrap().to_string();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "new plan not discovered"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(!f.root.join("runs/new-event/execution.lock").exists());
    let result = Command::new(&runner)
        .args(["resume", "--out"])
        .arg(f.root.join("runs/new-event"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let page = loop {
        let response = get(
            &addr,
            &format!("/api/events/{id}?q=new&map=0&outcome=draw&limit=1&offset=1&order=schedule"),
            &token,
        );
        let page = arena_runner::tournament::archive::parse_json(
            std::str::from_utf8(body(&response)).unwrap(),
        )
        .unwrap();
        if page.get("completed").unwrap().as_int() == Some(2) {
            break page;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(page.get("matched").unwrap().as_int(), Some(2));
    let matches = page.get("matches").unwrap().as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].get("index").unwrap().as_int(), Some(1));
    let response = get(
        &addr,
        &format!("/api/events/{id}/matches/1/attempts/0001/replay"),
        &token,
    );
    assert!(response.starts_with(b"HTTP/1.1 200"));
    assert_eq!(
        body(&response),
        fs::read(
            f.root
                .join("runs/new-event/matches/m1/attempts/0001/server/replay.iarp")
        )
        .unwrap()
    );
    for query in [
        "limit=0",
        "limit=201",
        "offset=bad",
        "limit=1&limit=2",
        "q=%ZZ",
    ] {
        assert!(
            get(&addr, &format!("/api/events/{id}?{query}"), &token).starts_with(b"HTTP/1.1 400")
        );
    }
    fs::create_dir_all(f.root.join("runs/unfinished/plan")).unwrap();
    let another = Command::new(&runner)
        .args(["plan", "--out"])
        .arg(f.root.join("runs/another-event"))
        .arg("--client")
        .arg(&bot)
        .arg("--client")
        .arg(&bot)
        .args(["--rounds", "1", "--fast"])
        .output()
        .unwrap();
    assert!(another.status.success());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let r = get(&addr, "/api/events", &token);
        let catalog =
            arena_runner::tournament::archive::parse_json(std::str::from_utf8(body(&r)).unwrap())
                .unwrap();
        let events = catalog.get("events").unwrap().as_array().unwrap();
        if events.len() == 3 {
            assert_eq!(
                events
                    .iter()
                    .filter(
                        |e| e.get("error") == Some(&arena_runner::tournament::archive::Json::Null)
                    )
                    .count(),
                2
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
    fs::rename(f.root.join("runs"), f.root.join("temporarily-unmounted")).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let response = get(&addr, "/api/events", &token);
        let catalog = arena_runner::tournament::archive::parse_json(
            std::str::from_utf8(body(&response)).unwrap(),
        )
        .unwrap();
        if !catalog
            .get("errors")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
        {
            let events = catalog.get("events").unwrap().as_array().unwrap();
            assert_eq!(
                events.len(),
                3,
                "unavailable source must keep a clearly stale snapshot"
            );
            assert_ne!(
                events[0].get("error"),
                Some(&arena_runner::tournament::archive::Json::Null)
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn rejects_cross_origin_unauthorized_writes_and_file_escape_but_keeps_serving() {
    let mut f = Fixture::new();
    fs::create_dir(f.root.join("web/assets")).unwrap();
    fs::write(f.root.join("secret.js"), "OUTSIDE SECRET").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        f.root.join("secret.js"),
        f.root.join("web/assets/linked.js"),
    )
    .unwrap();
    let (addr, token) = f.start();
    for (path, expected) in [
        ("/api/events", 401),
        ("/plan/tournament.json", 404),
        ("/assets/../secret.js", 400),
        ("/assets/%2e%2e/secret.js", 400),
        ("/assets/linked.js", 404),
    ] {
        let r = get(&addr, path, "");
        assert!(
            r.starts_with(format!("HTTP/1.1 {expected}").as_bytes()),
            "{}",
            String::from_utf8_lossy(&r)
        );
        assert!(!String::from_utf8_lossy(&r).contains("OUTSIDE SECRET"));
    }
    for (method, extra, host, code) in [
        ("POST", "", addr.as_str(), 405),
        ("GET", "Origin: http://evil.example\r\n", addr.as_str(), 403),
        ("GET", "", "evil.example", 403),
        ("GET", "Sec-Fetch-Site: cross-site\r\n", addr.as_str(), 403),
        ("GET", "Transfer-Encoding: chunked\r\n", addr.as_str(), 400),
    ] {
        let r = request(
            &addr,
            &format!(
                "{method} /api/events HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\n{extra}\r\n"
            ),
        );
        assert!(r.starts_with(format!("HTTP/1.1 {code}").as_bytes()));
        assert!(!String::from_utf8_lossy(&r).contains("Access-Control-Allow-Origin"));
    }
    // A partial/slow request occupies only one bounded worker, not the scanner
    // or every browser request. No sleeps needed to exercise that condition.
    let mut slow = TcpStream::connect(&addr).unwrap();
    slow.write_all(b"GET / HTTP/1.1\r\n").unwrap();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| assert!(get(&addr, "/api/events", &token).starts_with(b"HTTP/1.1 200")));
        }
    });
}

#[test]
fn cli_rejects_non_loopback_and_directory_in_place_of_index() {
    let f = Fixture::new();
    let run = |listen: &str| {
        Command::new(env!("CARGO_BIN_EXE_arena-viewer"))
            .args(["serve", "--runs"])
            .arg(f.root.join("runs"))
            .arg("--assets")
            .arg(f.root.join("web"))
            .args(["--listen", listen])
            .output()
            .unwrap()
    };
    assert!(!run("0.0.0.0:0").status.success());
    fs::remove_file(f.root.join("web/index.html")).unwrap();
    fs::create_dir(f.root.join("web/index.html")).unwrap();
    // Do not wait on an accidentally started daemon; poll its exit with a bound.
    let mut child = Command::new(env!("CARGO_BIN_EXE_arena-viewer"))
        .args(["serve", "--runs"])
        .arg(f.root.join("runs"))
        .arg("--assets")
        .arg(f.root.join("web"))
        .args(["--listen", "127.0.0.1:0"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let rejected = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break !status.success();
        }
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(rejected, "index.html must be a regular file at startup");
}
