//! Strict JSON boundary helpers. No additional dependency in the audited closure.
use super::archive::{Json, json_escape, parse_json};
use std::{io, path::Path};
pub(crate) fn invalid(s: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.to_string())
}
pub(crate) fn read(path: &Path) -> io::Result<Json> {
    parse_json(&std::fs::read_to_string(path)?).map_err(invalid)
}
pub(crate) fn text<'a>(j: &'a Json, key: &str) -> io::Result<&'a str> {
    j.get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| invalid(format!("{key}: expected string")))
}
pub(crate) fn array<'a>(j: &'a Json, key: &str) -> io::Result<&'a [Json]> {
    j.get(key)
        .and_then(Json::as_array)
        .ok_or_else(|| invalid(format!("{key}: expected array")))
}
pub(crate) fn number(j: &Json, key: &str) -> io::Result<u32> {
    j.get(key)
        .and_then(Json::as_int)
        .and_then(|n| n.try_into().ok())
        .ok_or_else(|| invalid(format!("{key}: expected u32")))
}
pub(crate) fn seed(j: &Json, key: &str) -> io::Result<u64> {
    let s = text(j, key)?;
    u64::from_str_radix(
        s.strip_prefix("0x").unwrap_or(s),
        if s.starts_with("0x") { 16 } else { 10 },
    )
    .map_err(invalid)
}
pub(crate) fn keys(j: &Json, allowed: &[&str]) -> io::Result<()> {
    let fields = j.as_object().ok_or_else(|| invalid("expected object"))?;
    let mut seen = std::collections::BTreeSet::new();
    for (key, _) in fields {
        if !allowed.contains(&key.as_str()) || !seen.insert(key) {
            return Err(invalid(format!("unknown or duplicate field: {key}")));
        }
    }
    Ok(())
}
pub(crate) fn string(s: impl ToString) -> Json {
    Json::Str(s.to_string())
}
pub(crate) fn set(j: &mut Json, key: &str, value: Json) -> io::Result<()> {
    let Json::Object(fields) = j else {
        return Err(invalid("expected object"));
    };
    if let Some((_, v)) = fields.iter_mut().find(|(k, _)| k == key) {
        *v = value;
    } else {
        fields.push((key.into(), value));
    }
    Ok(())
}
pub(crate) fn object(fields: Vec<(&str, Json)>) -> Json {
    Json::Object(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub(crate) fn encode(j: &Json) -> String {
    match j {
        Json::Null => "null".into(),
        Json::Bool(b) => b.to_string(),
        Json::Int(n) => n.to_string(),
        Json::Str(s) => format!("\"{}\"", json_escape(s)),
        Json::Array(a) => format!("[{}]", a.iter().map(encode).collect::<Vec<_>>().join(",")),
        Json::Object(a) => format!(
            "{{{}}}",
            a.iter()
                .map(|(k, v)| format!("{}:{}", encode(&string(k)), encode(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
pub(crate) fn identifier(s: &str) -> io::Result<()> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(invalid(
            "ID must be 1..128 ASCII letters, digits, '_' or '-'",
        ));
    }
    Ok(())
}
// Field names are the public GameConfig interface; codec ordering stays in Core.
macro_rules! config_fields {
    ($action:ident, $($arg:ident),*) => { $action!($($arg),*;
        total_rounds, initial_spawn_window_ms, decision_window_ms, priority_bucket_ms, epoch_length,
        map_width_cm, map_height_cm, portal_count, portal_min_distance_cm, clustered_portal_ratio_permille,
        portal_cluster_count, initial_agents_per_faction, reinforcement_interval_rounds, reinforcement_agents_per_faction,
        move_budget_cm, portal_operation_range_cm, transfer_range_cm, initial_supply, max_supply,
        portal_max_hp, attack_damage, recharge_amount, recharge_cost_supply, hack_supply_yield,
        hack_key_yield_min, hack_key_yield_max, hack_cooldown_rounds, hack_limit_per_epoch,
        max_link_distance_cm, max_commands_per_agent_round, max_frame_payload_bytes) };
}
pub(crate) fn config_json(c: &arena_core::config::GameConfig) -> Json {
    macro_rules! emit { ($c:ident; $($f:ident),*) => { object(vec![$((stringify!($f),Json::Int($c.$f as i64))),*]) }; }
    config_fields!(emit, c)
}
pub(crate) fn config(j: &Json) -> io::Result<arena_core::config::GameConfig> {
    let mut c = arena_core::config::GameConfig::default();
    macro_rules! parse { ($c:ident,$j:ident; $($f:ident),*) => {{
        keys($j,&[$(stringify!($f)),*])?;
        $(if let Some(v) = $j.get(stringify!($f)) { $c.$f = v.as_int().and_then(|n| n.try_into().ok()).ok_or_else(||invalid(concat!(stringify!($f),": integer out of range")))?; })*
    }}; }
    config_fields!(parse, c, j);
    c.validate().map_err(|e| invalid(format!("rules: {e:?}")))?;
    Ok(c)
}
