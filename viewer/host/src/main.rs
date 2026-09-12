//! Standalone read-only tournament host. No runner processes are started.
#![forbid(unsafe_code)]
mod catalog;
mod http;
use std::{
    io,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

struct Options {
    roots: Vec<PathBuf>,
    assets: PathBuf,
    listen: SocketAddr,
    poll: Duration,
}
impl Options {
    fn parse() -> Result<Self, String> {
        let mut args = std::env::args().skip(1);
        if args.next().as_deref() != Some("serve") {
            return Err("usage: arena-viewer serve --runs DIR [--runs DIR ...] [--assets viewer/app/dist] [--listen 127.0.0.1:8080] [--poll-ms 2000]".into());
        }
        let mut roots = Vec::new();
        let mut assets = PathBuf::from("viewer/app/dist");
        let mut listen: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let mut poll_ms = 2000;
        let mut seen = std::collections::BTreeSet::new();
        while let Some(key) = args.next() {
            if key != "--runs" && !seen.insert(key.clone()) {
                return Err(format!("duplicate {key}"));
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{key} requires a value"))?;
            match key.as_str() {
                "--runs" => roots.push(
                    PathBuf::from(value)
                        .canonicalize()
                        .map_err(|e| e.to_string())?,
                ),
                "--assets" => assets = value.into(),
                "--listen" => listen = value.parse().map_err(|_| "invalid IP:port")?,
                "--poll-ms" => {
                    poll_ms = value.parse::<u64>().map_err(|_| "invalid poll interval")?
                }
                _ => return Err(format!("unknown option {key}")),
            }
        }
        if roots.is_empty() || roots.iter().any(|p| !p.is_dir()) {
            return Err("--runs requires an existing directory".into());
        }
        if !listen.ip().is_loopback() {
            return Err(
                "only loopback addresses are supported; use an SSH tunnel for remote access".into(),
            );
        }
        if !(100..=60_000).contains(&poll_ms) {
            return Err("poll interval must be 100..60000 ms".into());
        }
        assets = assets
            .canonicalize()
            .map_err(|e| format!("build Viewer assets first: {e}"))?;
        let index = arena_runner::tournament::monitor::guarded_path(
            &assets,
            std::path::Path::new("index.html"),
        )
        .map_err(|e| e.to_string())?;
        if !index.is_file() {
            return Err("index.html must be a regular file".into());
        }
        Ok(Self {
            roots,
            assets,
            listen,
            poll: Duration::from_millis(poll_ms),
        })
    }
}
fn run(o: Options) -> io::Result<()> {
    let listener = TcpListener::bind(o.listen)?;
    let addr = listener.local_addr()?;
    let token = arena_runner::tournament::exec_external::execution_id()?;
    let published = Arc::new(RwLock::new(catalog::Published::default()));
    let target = published.clone();
    std::thread::spawn(move || {
        let mut catalog = catalog::Catalog::new(o.roots);
        loop {
            let update = catalog.scan();
            *target.write().unwrap() = update;
            std::thread::sleep(o.poll);
        }
    });
    let host = Arc::new(http::Host {
        assets: o.assets,
        authority: addr.to_string(),
        token: token.clone(),
        published,
    });
    println!("http://{addr}/#token={token}");
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let listener = &listener;
            let host = &host;
            scope.spawn(move || {
                for incoming in listener.incoming() {
                    match incoming {
                        Ok(stream) => {
                            let _ = host.connection(stream);
                        }
                        Err(e) => eprintln!("viewer accept: {e}"),
                    }
                }
            });
        }
    });
    Ok(())
}
fn main() -> std::process::ExitCode {
    let result = Options::parse().and_then(|o| run(o).map_err(|e| e.to_string()));
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("arena-viewer: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
