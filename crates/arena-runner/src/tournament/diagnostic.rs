//! Export a local, fixed-observation regression scenario; never label it a
//! re-decided match. SDK clients can consume it via the optional offline API.
use super::{archive::Json, document::*};
use arena_client_sdk::{
    offline::{Budgets, reconstruct},
    strategy::{Decision, Strategy},
};
use arena_core::{observe::Observation, types::Faction};
use std::{fs, io, path::Path};

struct NoDecision;
impl Strategy for NoDecision {
    fn decide(
        &mut self,
        _: &Observation,
        _: &mut dyn Decision,
    ) -> Result<(), arena_client_sdk::SdkError> {
        Ok(())
    }
}

pub fn execute_cli(args: &[String]) -> io::Result<Option<String>> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    if !matches!(verb, "scenario" | "diagnose") {
        return Ok(None);
    }
    let mut options = std::collections::BTreeMap::new();
    let mut pairs = args[1..].chunks_exact(2);
    for pair in &mut pairs {
        let allowed: &[&str] = if verb == "scenario" {
            &["--input", "--out", "--faction", "--budgets"]
        } else {
            &["--input", "--against"]
        };
        if !allowed.contains(&pair[0].as_str())
            || options.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err(invalid("unknown or duplicate option"));
        }
    }
    if !pairs.remainder().is_empty() {
        return Err(invalid("option requires a value"));
    }
    let get = |name| {
        options
            .get(name)
            .copied()
            .ok_or_else(|| invalid(format!("{name} required")))
    };
    if verb == "diagnose" {
        let a = read(Path::new(get("--input")?))?;
        let b = read(Path::new(get("--against")?))?;
        for key in [
            "schema",
            "source_hash",
            "budget_input_hash",
            "faction",
            "scope",
        ] {
            if text(&a, key)? != text(&b, key)? {
                return Err(invalid(format!("incompatible {key}")));
            }
        }
        if text(&a, "schema")? != "arena-decisions-v1" {
            return Err(invalid("unsupported decisions schema"));
        }
        let ar = array(&a, "rounds")?;
        let br = array(&b, "rounds")?;
        let mut first = Json::Null;
        for i in 0..ar.len().max(br.len()) {
            let differs = match (ar.get(i), br.get(i)) {
                (Some(a), Some(b)) => {
                    if number(a, "round")? as usize != i + 1
                        || number(b, "round")? as usize != i + 1
                    {
                        return Err(invalid("history must begin at opening without gaps"));
                    }
                    for key in ["public_snapshot_hex", "private_snapshot_hex"] {
                        if text(a, key)? != text(b, key)? {
                            return Err(invalid("observation history differs"));
                        }
                    }
                    array(a, "commands")? != array(b, "commands")?
                }
                _ => true,
            };
            if differs && first == Json::Null {
                first = Json::Int(i as i64 + 1);
            }
        }
        return Ok(Some(encode(&object(vec![
            ("schema", string("arena-diagnosis-v1")),
            ("scope", string("recorded-observations")),
            ("first_difference", first),
            ("rounds_a", Json::Int(ar.len() as i64)),
            ("rounds_b", Json::Int(br.len() as i64)),
        ]))));
    }
    let bytes = fs::read(get("--input")?)?;
    let log = arena_replay::format::decode(&bytes).map_err(invalid)?;
    let faction = match get("--faction")? {
        "enl" => Faction::Enl,
        "res" => Faction::Res,
        _ => return Err(invalid("faction must be enl or res")),
    };
    let budgets = if let Some(path) = options.get("--budgets") {
        fs::read_to_string(path)?
    } else {
        let mut out = String::from("arena-budgets-v1\n");
        for r in log.round_records() {
            out.push_str(&format!("{}\t\n", r.round));
        }
        out
    };
    let parsed = Budgets::parse(&budgets)?;
    let history = reconstruct(&log, faction, &parsed, &mut NoDecision)?;
    let root = Path::new(get("--out")?);
    fs::create_dir(root)?;
    super::durable::atomic_write(&root.join("source.iarp"), &bytes)?;
    super::durable::atomic_write(&root.join("faction"), get("--faction")?.as_bytes())?;
    super::durable::atomic_write(&root.join("budgets.tsv"), budgets.as_bytes())?;
    super::durable::atomic_write(
        &root.join("recorded.json"),
        history.to_json(true).as_bytes(),
    )?;
    super::durable::atomic_write(&root.join("README.md"), b"# Recorded-observation scenario\n\nSource replay is verified. Start the strategy at opening. New commands do not change future observations. budgets.tsv is explicit injected input, empty means no clock reads. recorded.json holds ordered recorded submissions, not a requirement that a new strategy reproduce a failure. Save an approved strategy output separately as a regression expectation. No automatic mid-game Bot state restore or timing equivalence.\n")?;
    Ok(Some(format!(
        "scenario={} rounds={}\n",
        root.display(),
        history.rounds.len()
    )))
}
