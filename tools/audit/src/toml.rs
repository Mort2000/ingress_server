//! Minimal Cargo.toml section parser (D73): extracts dependency entries
//! from the sections that can pull code into an audited build. No TOML
//! crate — the audit must stay out of the audited closure, and the
//! Cargo manifest grammar it needs is deliberately tiny.

/// A dependency entry: its name and the raw (joined) value text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepEntry {
    pub name: String,
    pub value: String,
    pub section: String,
}

/// The dependency table kinds whose entries are code sources.
const DEP_KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Parses every dependency entry in the manifest.
///
/// Entries may span lines (inline tables); an entry ends where the next
/// `name =` starts at brace depth 0 or the section ends. Every spelling
/// cargo accepts for a dependency table is recognized: plain sections
/// (`[dependencies]`), DOTTED single-entry tables
/// (`[dependencies.evilgit]` — the following key lines are that one
/// entry's value), cfg target sections
/// (`[target.'cfg(unix)'.dependencies]`, normalized
/// `target.cfg.dependencies`), TRIPLE target sections
/// (`[target.x86_64-unknown-linux-gnu.dependencies]`, kept verbatim),
/// dotted variants of both, and the `workspace.*` tables.
pub fn dependency_entries(manifest: &str) -> Vec<DepEntry> {
    let mut entries = Vec::new();
    let mut section: Option<String> = None;
    let mut pending: Option<(String, String)> = None;
    // Inside a dotted header (`[dependencies.name]`): every key line
    // belongs to that single entry.
    let mut dotted = false;

    macro_rules! flush {
        () => {
            if let Some((name, value)) = pending.take() {
                if let Some(section) = &section {
                    entries.push(DepEntry {
                        name,
                        value,
                        section: section.clone(),
                    });
                }
            }
        };
    }

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            match classify_header(&line[1..line.len() - 1]) {
                Some((kind, entry_name)) => {
                    flush!();
                    section = Some(kind);
                    dotted = entry_name.is_some();
                    if let Some(name) = entry_name {
                        pending = Some((name, String::new()));
                    }
                }
                None => {
                    // Another table header ends the previous section.
                    // (A bracketed line that is not a table header —
                    // it carries an `=` — is inert noise.)
                    if !line.contains('=') {
                        flush!();
                        section = None;
                        dotted = false;
                    }
                }
            }
            continue;
        }
        if section.is_some() {
            let in_open_table = pending
                .as_ref()
                .is_some_and(|(_, value)| brace_depth(value) > 0);
            if in_open_table {
                // Continuation of a multi-line inline table.
                if let Some((_, value)) = pending.as_mut() {
                    value.push('\n');
                    value.push_str(line);
                }
                continue;
            }
            if dotted {
                // Key line of a `[section.name]` table: joins the
                // pending entry's value so the source checks (`git =`,
                // `path =`, ...) see it.
                if let Some((_, value)) = pending.as_mut() {
                    if !value.is_empty() {
                        value.push('\n');
                    }
                    value.push_str(line);
                }
                continue;
            }
            if let Some((name, value)) = entry_start(line) {
                flush!();
                pending = Some((name.to_string(), value.to_string()));
            } else if let Some((_, value)) = pending.as_mut() {
                value.push('\n');
                value.push_str(line);
            }
        }
    }
    flush!();
    entries
}

/// Classifies a table header body into `(section, Some(entry name))`
/// for dotted single-entry tables, `(section, None)` for plain tables,
/// or `None` when the table is not a dependency section. `cfg(...)`
/// target predicates collapse to `cfg` (stable section labels).
fn classify_header(inner: &str) -> Option<(String, Option<String>)> {
    let cleaned = inner.replace(['\'', '"'], "");
    let parts: Vec<&str> = cleaned.split('.').filter(|p| !p.is_empty()).collect();
    if parts.first() == Some(&"workspace") {
        if parts.len() >= 2 && DEP_KINDS.contains(&parts[1]) {
            return Some((
                format!("workspace.{}", parts[1]),
                parts.get(2).map(|name| name.to_string()),
            ));
        }
        return None;
    }
    if parts.first() == Some(&"target") && parts.len() >= 3 {
        let last = parts.len() - 1;
        let mid = |end: usize| -> String {
            parts[1..end]
                .iter()
                .map(|p| if p.starts_with("cfg(") { "cfg" } else { p })
                .collect::<Vec<_>>()
                .join(".")
        };
        // [target.<cfg-or-triple>.dependencies]
        if DEP_KINDS.contains(&parts[last]) {
            return Some((format!("target.{}.{}", mid(last), parts[last]), None));
        }
        // [target.<cfg-or-triple>.dependencies.name]
        if DEP_KINDS.contains(&parts[last - 1]) {
            return Some((
                format!("target.{}.{}", mid(last - 1), parts[last - 1]),
                Some(parts[last].to_string()),
            ));
        }
        return None;
    }
    if let Some(first) = parts.first() {
        if DEP_KINDS.contains(first) {
            return Some((first.to_string(), parts.get(1).map(|name| name.to_string())));
        }
    }
    None
}

/// Extracts the quoted string value of `key` from an entry's value text
/// (`git = "..."`, `path = "../x"` — spacing-tolerant, word-boundary
/// keyed so `features = ["git-cli"]` does not match `git`), if present.
pub fn string_value(value: &str, key: &str) -> Option<String> {
    let hay = value.as_bytes();
    let needle = key.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let mut i = 0usize;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] != needle {
            i += 1;
            continue;
        }
        let before_ok = i == 0 || !is_key_byte(hay[i - 1]);
        let mut j = i + needle.len();
        while j < hay.len() && (hay[j] == b' ' || hay[j] == b'\t') {
            j += 1;
        }
        if before_ok && j < hay.len() && hay[j] == b'=' {
            j += 1;
            while j < hay.len() && (hay[j] == b' ' || hay[j] == b'\t') {
                j += 1;
            }
            if j < hay.len() && hay[j] == b'"' {
                let start = j + 1;
                let mut k = start;
                while k < hay.len() && hay[k] != b'"' {
                    if hay[k] == b'\\' {
                        k += 1;
                    }
                    k += 1;
                }
                if k < hay.len() {
                    return Some(String::from_utf8_lossy(&hay[start..k]).into_owned());
                }
            }
        }
        i += needle.len().max(1);
    }
    None
}

fn is_key_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Net `{`/`[` depth of a (possibly multi-line) entry value.
fn brace_depth(value: &str) -> i32 {
    let mut depth = 0i32;
    for c in value.chars() {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// `name = value` at brace depth 0 (value may be empty when the inline
/// table continues on later lines).
fn entry_start(line: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (idx, c) in line.char_indices() {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            '=' if depth == 0 => {
                let name = line[..idx].trim();
                let value = line[idx + 1..].trim();
                if is_dep_name(name) {
                    return Some((name, value));
                }
                return None;
            }
            _ => {}
        }
    }
    None
}

fn is_dep_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Reads a scalar/array key from a table section, e.g.
/// `table_value(manifest, "lib", "crate-type")` for
/// `[lib] crate-type = ["cdylib"]` (returns the raw joined text).
pub fn table_value(manifest: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') && !line.contains('=') {
            in_section = line[1..line.len() - 1].split('.').next() == Some(section);
            continue;
        }
        if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// The `[patch]`/`[replace]` source-substitution tables: any use needs
/// explicit review (they can rewrite closure sources behind the lock).
pub fn has_patch_or_replace(manifest: &str) -> bool {
    manifest.lines().any(|raw| {
        let line = raw.trim();
        (line.starts_with("[patch") || line.starts_with("[replace")) && line.ends_with(']')
    })
}
