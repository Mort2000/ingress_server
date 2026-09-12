//! Local admission control. Reservations are NOT CPU or RSS hard limits.
//! Host slot locks coordinate cooperating runners and release on process death.
use super::{archive::Json, document::*, durable::Plan};
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct Resources {
    pub workers: usize,
    pub report: Json,
    pool: Option<(PathBuf, usize)>,
}
pub struct Permit {
    _lock: Option<File>,
    pub slot: Option<usize>,
}

pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

/// Point-in-time host observations, not a continuous profiler.
pub fn host_sample() -> Json {
    object(vec![
        ("sampled_unix_ms", Json::Int(unix_ms() as i64)),
        (
            "loadavg",
            string(
                fs::read_to_string("/proc/loadavg")
                    .unwrap_or_else(|_| "unavailable".into())
                    .trim(),
            ),
        ),
        (
            "meminfo",
            string(fs::read_to_string("/proc/meminfo").unwrap_or_else(|_| "unavailable".into())),
        ),
    ])
}
fn positive(j: &Json, key: &str) -> io::Result<u64> {
    let value = match j.get(key) {
        Some(Json::Str(s)) => s.parse().map_err(invalid)?,
        Some(Json::Int(n)) if *n > 0 => *n as u64,
        _ => {
            return Err(invalid(format!(
                "{key} must be a positive integer or decimal string"
            )));
        }
    };
    if value == 0 {
        return Err(invalid(format!("{key} must be positive")));
    }
    Ok(value)
}
impl Resources {
    pub fn inspect(plan: &Plan) -> io::Result<Self> {
        let declared = plan.extension.as_ref().and_then(|e| e.get("resources"));
        let cpus = std::thread::available_parallelism()?.get();
        let mut workers = plan.config.concurrency;
        let mut pool = None;
        if let Some(r) = declared {
            if r.get("cpu_slots").is_some() {
                workers = workers.min(usize::try_from(positive(r, "cpu_slots")?).map_err(invalid)?);
            }
            if r.get("memory_budget_bytes").is_some() {
                // Reserve both client's declared address spaces plus 64 MiB
                // for server/tooling. Actual RSS/cgroup enforcement is separate.
                let per_match = positive(r, "address_space_bytes")?
                    .checked_mul(2)
                    .and_then(|v| v.checked_add(64 * 1024 * 1024))
                    .ok_or_else(|| invalid("memory reservation overflow"))?;
                workers = workers.min(
                    usize::try_from(positive(r, "memory_budget_bytes")? / per_match)
                        .map_err(invalid)?,
                );
            }
            if let Some(value) = r.get("host_pool") {
                let path = PathBuf::from(
                    value
                        .as_str()
                        .ok_or_else(|| invalid("host_pool must be an absolute path"))?,
                );
                if !path.is_absolute() {
                    return Err(invalid("host_pool must be absolute"));
                }
                let manifest = read(&path.join("pool.json"))?;
                if text(&manifest, "schema")? != "arena-resource-pool-v1" {
                    return Err(invalid("unknown pool schema"));
                }
                let slots = number(&manifest, "slots")? as usize;
                if slots == 0 || slots > 4096 {
                    return Err(invalid("pool slots must be 1..4096"));
                }
                workers = workers.min(slots);
                pool = Some((path.canonicalize()?, slots));
            }
        }
        if workers == 0 {
            return Err(invalid("resource budget cannot admit one match"));
        }
        let report = object(vec![
            (
                "mode",
                plan.extension
                    .as_ref()
                    .and_then(|e| e.get("mode"))
                    .cloned()
                    .unwrap_or(string("legacy-unspecified")),
            ),
            ("schema", string("arena-resource-preflight-v1")),
            ("scheduler", string("dynamic")),
            (
                "declared_workers",
                Json::Int(plan.config.concurrency as i64),
            ),
            ("effective_workers", Json::Int(workers as i64)),
            ("available_parallelism", Json::Int(cpus as i64)),
            ("cpu_hard_enforced", Json::Bool(false)),
            ("batch_memory_hard_enforced", Json::Bool(false)),
            (
                "client_address_space_requested",
                Json::Bool(!matches!(plan.config.sandbox, super::ClientSandbox::Off)),
            ),
            (
                "isolation_evidence",
                string("per-attempt sandbox reports; cgroup availability is not assumed"),
            ),
            (
                "host_coordination",
                string(if pool.is_some() {
                    "cooperating-runner-slot-locks"
                } else {
                    "none; operator must bound total host concurrency"
                }),
            ),
            ("declared", declared.cloned().unwrap_or(Json::Null)),
            ("preflight_sample", host_sample()),
        ]);
        Ok(Self {
            workers,
            report,
            pool,
        })
    }
    pub fn acquire(&self) -> io::Result<Permit> {
        let Some((root, slots)) = &self.pool else {
            return Ok(Permit {
                _lock: None,
                slot: None,
            });
        };
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            for slot in 0..*slots {
                let file = OpenOptions::new()
                    .write(true)
                    .open(root.join(format!("slot-{slot}.lock")))?;
                match file.try_lock() {
                    Ok(()) => {
                        return Ok(Permit {
                            _lock: Some(file),
                            slot: Some(slot),
                        });
                    }
                    Err(std::fs::TryLockError::WouldBlock) => (),
                    Err(std::fs::TryLockError::Error(e)) => return Err(e),
                }
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "host pool admission timed out; no attempt started",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

pub fn execute_cli(args: &[String]) -> io::Result<Option<String>> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    if verb == "preflight" {
        if args.len() != 3 || args[1] != "--out" {
            return Err(invalid("preflight --out PLAN_DIR"));
        }
        return Ok(Some(encode(
            &Resources::inspect(&super::durable::load(Path::new(&args[2]))?)?.report,
        )));
    }
    if verb != "resource-pool" {
        return Ok(None);
    }
    if args.len() != 5 || args[1] != "--out" || args[3] != "--slots" {
        return Err(invalid("resource-pool --out NEW_DIR --slots N"));
    }
    let slots: usize = args[4].parse().map_err(invalid)?;
    if slots == 0 || slots > 4096 {
        return Err(invalid("slots must be 1..4096"));
    }
    let root = Path::new(&args[2]);
    fs::create_dir(root)?;
    for slot in 0..slots {
        File::create_new(root.join(format!("slot-{slot}.lock")))?.sync_all()?;
    }
    super::durable::atomic_write(
        &root.join("pool.json"),
        encode(&object(vec![
            ("schema", string("arena-resource-pool-v1")),
            ("slots", Json::Int(slots as i64)),
        ]))
        .as_bytes(),
    )?;
    Ok(Some(format!(
        "pool={} slots={slots}\n",
        root.canonicalize()?.display()
    )))
}
