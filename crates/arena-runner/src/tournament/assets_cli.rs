//! Public file/CLI boundaries for map and build assets.
use super::{
    archive::Json,
    document::*,
    maps::{Layout, MapAsset},
};
use std::{collections::BTreeMap, io, path::Path};
pub(crate) fn options(args: &[String], allowed: &[&str]) -> io::Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    let mut pairs = args.chunks_exact(2);
    for pair in &mut pairs {
        if !allowed.contains(&pair[0].as_str())
            || result.insert(pair[0].clone(), pair[1].clone()).is_some()
        {
            return Err(invalid(format!("unknown or duplicate option {}", pair[0])));
        }
    }
    if !pairs.remainder().is_empty() {
        return Err(invalid("option requires value"));
    }
    Ok(result)
}
pub(crate) fn required<'a>(opts: &'a BTreeMap<String, String>, key: &str) -> io::Result<&'a str> {
    opts.get(key)
        .map(String::as_str)
        .ok_or_else(|| invalid(format!("{key} required")))
}
pub fn execute(args: &[String]) -> io::Result<Option<String>> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    if !matches!(
        verb,
        "build-register"
            | "build-inspect"
            | "map-generate"
            | "map-import"
            | "map-export"
            | "map-inspect"
            | "map-set"
            | "map-preview"
    ) {
        return Ok(None);
    }
    let allowed: &[&str] = match verb {
        "build-register" => &["--spec", "--out"],
        "build-inspect" | "map-inspect" => &["--input"],
        "map-generate" => &["--config", "--layout", "--seed", "--out"],
        "map-set" => &["--spec", "--out"],
        _ => &["--input", "--out"],
    };
    let opts = options(&args[1..], allowed)?;
    if verb == "build-register" {
        let j = super::builds::register(
            Path::new(required(&opts, "--spec")?),
            Path::new(required(&opts, "--out")?),
        )?;
        return Ok(Some(encode(&j)));
    }
    if verb == "build-inspect" {
        return Ok(Some(encode(&super::builds::inspect(Path::new(required(
            &opts, "--input",
        )?))?)));
    }
    if verb == "map-set" {
        let spec = Path::new(required(&opts, "--spec")?);
        let j = read(spec)?;
        keys(&j, &["schema", "purpose", "maps"])?;
        if text(&j, "schema")? != "arena-map-set-request-v1" {
            return Err(invalid("unsupported map-set request"));
        }
        let purpose = text(&j, "purpose")?;
        if !matches!(purpose, "development" | "regression" | "confirmation") {
            return Err(invalid("unknown map set purpose"));
        }
        let mut assets = Vec::new();
        for v in array(&j, "maps")? {
            let p = spec.parent().unwrap_or(Path::new(".")).join(
                v.as_str()
                    .ok_or_else(|| invalid("map path must be string"))?,
            );
            assets.push(MapAsset::from_json(&std::fs::read_to_string(p)?)?);
        }
        if assets.is_empty() {
            return Err(invalid("map set cannot be empty"));
        }
        let mut hashes = assets
            .iter()
            .map(|a| {
                format!(
                    "{}:{:016x}",
                    super::schedule::map_codec_hash(&a.map),
                    super::config::config_identity(&a.config)
                )
            })
            .collect::<Vec<_>>();
        hashes.sort();
        if hashes.windows(2).any(|a| a[0] == a[1]) {
            return Err(invalid("duplicate map and rules content in set"));
        }
        let set = object(vec![
            ("schema", string("arena-map-set-v1")),
            ("purpose", string(purpose)),
            (
                "content_id",
                string(blake3::hash(hashes.join("\n").as_bytes()).to_hex()),
            ),
            (
                "maps",
                Json::Array(
                    assets
                        .iter()
                        .map(|a| super::archive::parse_json(&a.to_json()).unwrap())
                        .collect(),
                ),
            ),
        ]);
        write_new(
            Path::new(required(&opts, "--out")?),
            encode(&set).as_bytes(),
        )?;
        return Ok(Some(encode(&set)));
    }
    let asset = if verb == "map-generate" {
        MapAsset::generate(
            config(&read(Path::new(required(&opts, "--config")?))?)?,
            Layout::parse(required(&opts, "--layout")?)?,
            required(&opts, "--seed")?.parse::<u64>().map_err(invalid)?,
        )?
    } else {
        MapAsset::from_json(&std::fs::read_to_string(required(&opts, "--input")?)?)?
    };
    if verb == "map-preview" {
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"#101b24\"/><g transform=\"translate(0,{}) scale(1,-1)\" fill=\"#66e2ad\">",
            asset.config.map_width_cm, asset.config.map_height_cm, asset.config.map_height_cm
        );
        let radius = (asset.config.map_width_cm.max(asset.config.map_height_cm) / 200).max(1);
        for p in asset.map.portals() {
            svg.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{radius}\"><title>Portal {}</title></circle>",
                p.position.x, p.position.y, p.id.0
            ));
        }
        svg.push_str("</g></svg>\n");
        write_new(Path::new(required(&opts, "--out")?), svg.as_bytes())?;
    } else if verb != "map-inspect" {
        write_new(
            Path::new(required(&opts, "--out")?),
            asset.to_json().as_bytes(),
        )?;
    }
    let summary = asset.summary();
    let n = asset.map.len() as u64;
    Ok(Some(encode(&object(vec![
        ("hash", string(super::schedule::map_codec_hash(&asset.map))),
        ("portals", Json::Int(n as i64)),
        ("edges", Json::Int(summary.edges as i64)),
        ("possible_edges", Json::Int((n * (n - 1) / 2) as i64)),
        ("triangles", Json::Int(summary.triangles as i64)),
        ("components", Json::Int(summary.components as i64)),
        (
            "warnings",
            Json::Array(summary.warnings.iter().map(string).collect()),
        ),
    ]))))
}
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::File::open(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?
    .sync_all()
}
