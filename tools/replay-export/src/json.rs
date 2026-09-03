//! D92 mini-JSON: a std-only compact writer plus a strict parser for the
//! embedded viewer payload and the optional `meta.json` echo.
//!
//! The M9 hand-rolled mini-JSON precedent (`arena-runner` tournament
//! archive) stays untouched; this crate owns its own copy because the
//! viewer payload has one extra, load-bearing requirement: besides the
//! JSON-mandated escapes the WRITER also escapes `<`, `>`, and `&` (as
//! `<`, `>`, `&`), so a hostile string inside a
//! `meta.json` can never contain the literal bytes `</script>` and break
//! out of the page's `<script>` element. The emitted document is COMPACT
//! (no whitespace anywhere); the parser accepts whitespace between tokens
//! (real `meta.json` files are pretty-printed).
//!
//! Numbers are INTEGERS ONLY: the parser rejects `.`/`e`/`E` outright
//! (D91 — floats never enter the embedded data; `score2` travels as
//! decimal strings where u128 exactness matters). Every integer the
//! exporter emits fits in i64 by construction (ids/coords/hp/supply are
//! u32/i32, `area2` < 2^53 by the D10 bounds).

use std::fmt;

/// Maximum container nesting the parser accepts and the writer emits
/// (M-final review fix). The viewer payload nests at most five levels
/// (root object -> header/rounds -> round -> arrays -> scalars); anything
/// deeper is hostile input, and a clean error beats recursing toward a
/// stack overflow on a hand-crafted `[[[[…`.
pub const MAX_DEPTH: usize = 256;

/// A mini-JSON value. Objects preserve insertion order (the schema's key
/// order is part of the frozen `ingress-viewer-v1` contract).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// String value constructor.
    pub fn str(text: impl Into<String>) -> Json {
        Json::Str(text.into())
    }

    /// Integer value constructor.
    pub fn int(value: i64) -> Json {
        Json::Int(value)
    }

    /// 64-bit unsigned constructor (values the exporter emits fit i64).
    pub fn uint(value: u64) -> Json {
        Json::Int(value as i64)
    }

    /// Array constructor.
    pub fn arr(items: Vec<Json>) -> Json {
        Json::Array(items)
    }

    /// Object constructor from ordered `(key, value)` pairs.
    pub fn obj(pairs: Vec<(&str, Json)>) -> Json {
        Json::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    /// First value stored under `key` (objects may repeat keys per JSON
    /// grammar; the writer never emits repeats).
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// String contents, if this is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(text) => Some(text),
            _ => None,
        }
    }

    /// Integer value, if this is an integer.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Json::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// Boolean value, if this is a bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Array items, if this is an array.
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Object pairs, if this is an object.
    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(pairs) => Some(pairs),
            _ => None,
        }
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Depth-safe by construction: an over-deep value renders as this
        // placeholder instead of recursing (write_json enforces MAX_DEPTH).
        f.write_str(&write_json(self).unwrap_or_else(|_| "\"<max depth exceeded>\"".into()))
    }
}

// ---------------------------------------------------------------------------
// Writer (compact; `<`, `>`, `&`, and controls escaped)
// ---------------------------------------------------------------------------

/// Writer-side failure: the value nests deeper than [`MAX_DEPTH`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DepthError {
    pub max_depth: usize,
}

impl fmt::Display for DepthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nesting exceeds the maximum depth {}", self.max_depth)
    }
}

impl std::error::Error for DepthError {}

/// Serializes `value` compactly. String contents escape the JSON-mandated
/// characters PLUS `<`, `>`, and `&` so the bytes can be embedded inside a
/// `<script>` element without any possibility of element breakout. Values
/// nested deeper than [`MAX_DEPTH`] return [`DepthError`] instead of
/// recursing toward a stack overflow.
pub fn write_json(value: &Json) -> Result<String, DepthError> {
    let mut out = String::new();
    write_value(value, 0, &mut out)?;
    Ok(out)
}

fn write_value(value: &Json, depth: usize, out: &mut String) -> Result<(), DepthError> {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(number) => out.push_str(&number.to_string()),
        Json::Str(text) => write_string(text, out),
        Json::Array(items) => {
            depth_check(depth)?;
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(item, depth + 1, out)?;
            }
            out.push(']');
        }
        Json::Object(pairs) => {
            depth_check(depth)?;
            out.push('{');
            for (index, (key, item)) in pairs.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_value(item, depth + 1, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn depth_check(depth: usize) -> Result<(), DepthError> {
    if depth >= MAX_DEPTH {
        Err(DepthError {
            max_depth: MAX_DEPTH,
        })
    } else {
        Ok(())
    }
}

fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // The script-breakout triples (D92): never emitted literally.
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

// ---------------------------------------------------------------------------
// Parser (strict: objects/arrays/strings/bools/null/integers; floats and
// trailing garbage rejected)
// ---------------------------------------------------------------------------

/// Every way parsing can fail; the message names the offset.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    pub message: String,
    pub at: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.at)
    }
}

impl std::error::Error for ParseError {}

/// Parses one JSON document. Whitespace between tokens is allowed (the
/// emitter never writes any, but `meta.json` inputs are pretty-printed);
/// anything after the document's root value is an error, float syntax is
/// rejected, raw control characters inside strings are rejected, and
/// container nesting deeper than [`MAX_DEPTH`] is a clean error instead of
/// unbounded recursion.
pub fn parse_json(text: &str) -> Result<Json, ParseError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        position: 0,
    };
    let value = parser.value(0)?;
    parser.skip_ws();
    if parser.position != parser.bytes.len() {
        return Err(parser.error("trailing bytes after the JSON document"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn error(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_string(),
            at: self.position,
        }
    }

    fn skip_ws(&mut self) {
        while let Some(byte) = self.bytes.get(self.position) {
            if byte.is_ascii_whitespace() {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn value(&mut self, depth: usize) -> Result<Json, ParseError> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(byte) if byte.is_ascii_digit() || byte == b'-' => self.number(),
            Some(other) => Err(self.error(&format!("unexpected byte {other:?}"))),
            None => Err(self.error("unexpected end of input")),
        }
    }

    fn literal(&mut self, text: &str, value: Json) -> Result<Json, ParseError> {
        if self.bytes[self.position..].starts_with(text.as_bytes()) {
            self.position += text.len();
            Ok(value)
        } else {
            Err(self.error("bad literal"))
        }
    }

    fn number(&mut self) -> Result<Json, ParseError> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position += 1;
        }
        let digits_start = self.position;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_digit() {
                self.position += 1;
            } else {
                break;
            }
        }
        if self.position == digits_start {
            return Err(self.error("number without digits"));
        }
        // D91/D92: floats are rejected outright — the embedded data is
        // integer-only.
        if matches!(self.peek(), Some(b'.') | Some(b'e') | Some(b'E')) {
            return Err(self.error("float rejected (integers only)"));
        }
        let text = std::str::from_utf8(&self.bytes[start..self.position])
            .map_err(|_| self.error("utf8"))?;
        text.parse::<i64>()
            .map(Json::Int)
            .map_err(|_| self.error("integer out of range"))
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.position += 1; // opening quote
        let mut out = String::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.position += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.position += 1;
                    match self.peek() {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => out.push('\u{0008}'),
                        Some(b'f') => out.push('\u{000C}'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => {
                            self.position += 1;
                            let hex = std::str::from_utf8(
                                self.bytes
                                    .get(self.position..self.position + 4)
                                    .ok_or_else(|| self.error("truncated \\u escape"))?,
                            )
                            .map_err(|_| self.error("utf8"))?;
                            let code =
                                u32::from_str_radix(hex, 16).map_err(|_| self.error("bad \\u"))?;
                            out.push(
                                char::from_u32(code).ok_or_else(|| self.error("bad \\u code"))?,
                            );
                            self.position += 3; // +1 for the peek advance below
                        }
                        other => return Err(self.error(&format!("bad escape {other:?}"))),
                    }
                    self.position += 1;
                }
                Some(control) if control < 0x20 => {
                    return Err(self.error("raw control character in string"));
                }
                Some(_) => {
                    let text = std::str::from_utf8(&self.bytes[self.position..])
                        .map_err(|_| self.error("utf8"))?;
                    let character = text.chars().next().expect("non-empty slice has a char");
                    out.push(character);
                    self.position += character.len_utf8();
                }
                None => return Err(self.error("unterminated string")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, ParseError> {
        self.depth_check(depth)?;
        self.position += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.position += 1,
                Some(b']') => {
                    self.position += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']' in array")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, ParseError> {
        self.depth_check(depth)?;
        self.position += 1; // '{'
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(Json::Object(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected string key in object"));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':' after object key"));
            }
            self.position += 1;
            pairs.push((key, self.value(depth + 1)?));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.position += 1,
                Some(b'}') => {
                    self.position += 1;
                    return Ok(Json::Object(pairs));
                }
                _ => return Err(self.error("expected ',' or '}' in object")),
            }
        }
    }

    fn depth_check(&self, depth: usize) -> Result<(), ParseError> {
        if depth >= MAX_DEPTH {
            Err(self.error("nesting exceeds the maximum depth"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // D92: writer -> parser identity across the whole type surface.
    #[test]
    fn round_trip_identity() {
        let value = Json::obj(vec![
            ("format", Json::str("ingress-viewer-v1")),
            ("n", Json::int(-42)),
            ("u", Json::uint(4_294_967_295)),
            ("flag", Json::Bool(true)),
            ("off", Json::Bool(false)),
            ("nothing", Json::Null),
            (
                "list",
                Json::arr(vec![
                    Json::int(1),
                    Json::str("two"),
                    Json::arr(vec![Json::int(3)]),
                    Json::obj(vec![("deep", Json::str("four"))]),
                ]),
            ),
            ("empty_arr", Json::arr(vec![])),
            ("empty_obj", Json::obj(vec![])),
        ]);
        let text = write_json(&value).expect("writes");
        assert_eq!(parse_json(&text).expect("parses"), value);
        // Structural identity is byte identity: writing the parse result
        // reproduces the exact compact document.
        assert_eq!(
            write_json(&parse_json(&text).expect("parses")).expect("writes"),
            text
        );
    }

    // D92: `<`, `>`, `&`, quotes, backslashes, and control characters are
    // escaped so a hostile string can never break out of `<script>` — in
    // particular the literal bytes `</script>` never appear in the output.
    #[test]
    fn script_breakout_is_impossible() {
        let hostile = "</script><script>alert(1)</script>&\"\\ \u{0001}\u{001f}\n\t";
        let text = write_json(&Json::str(hostile)).expect("writes");
        assert!(!text.contains("</script>"), "no literal close tag: {text}");
        assert!(
            !text.contains('<') && !text.contains('>'),
            "no raw angle brackets"
        );
        assert!(!text.contains('&'), "no raw ampersand");
        // And the parser restores the exact original string.
        assert_eq!(
            parse_json(&text).expect("parses"),
            Json::Str(hostile.to_string())
        );
        // A full document embedding the hostile string is equally inert.
        let doc = write_json(&Json::obj(vec![("meta", Json::str(hostile))])).expect("writes");
        assert!(!doc.contains("</script>"));
        assert_eq!(
            parse_json(&doc)
                .expect("parses")
                .get("meta")
                .and_then(Json::as_str),
            Some(hostile)
        );
    }

    // D92: floats are rejected on parse, in every position.
    #[test]
    fn floats_rejected_everywhere() {
        for text in [
            "1.5",
            "-0.0",
            "[1,2.5]",
            "{\"a\":1e3}",
            "{\"a\":2E-1}",
            "[1.0]",
        ] {
            let error = parse_json(text).expect_err("floats are rejected");
            assert!(
                error.message.contains("float"),
                "expected a float rejection for {text}, got {error}"
            );
        }
    }

    // D92: compact output — no whitespace anywhere, including between
    // object pairs and array items.
    #[test]
    fn output_is_compact() {
        let text = write_json(&Json::obj(vec![
            ("a", Json::int(1)),
            ("b", Json::arr(vec![Json::int(2), Json::int(3)])),
        ]))
        .expect("writes");
        assert_eq!(text, "{\"a\":1,\"b\":[2,3]}");
        assert!(!text.contains(' ') && !text.contains('\n') && !text.contains('\t'));
    }

    // The parser accepts pretty-printed input (real meta.json shape).
    #[test]
    fn parser_tolerates_pretty_printed_input() {
        let text = "{\n  \"schema\": \"ingress-tournament-match-v1\",\n  \"index\": 6,\n  \"ok\": true,\n  \"none\": null\n}\n";
        let value = parse_json(text).expect("pretty input parses");
        assert_eq!(
            value.get("schema").and_then(Json::as_str),
            Some("ingress-tournament-match-v1")
        );
        assert_eq!(value.get("index").and_then(Json::as_int), Some(6));
        assert_eq!(value.get("ok").and_then(Json::as_bool), Some(true));
        assert_eq!(value.get("none"), Some(&Json::Null));
    }

    // Malformed documents fail loudly instead of parsing something odd.
    #[test]
    fn malformed_documents_rejected() {
        for text in [
            "",
            "{",
            "}",
            "[1,]",
            "{\"a\"}",
            "{\"a\":}",
            "{\"a\":1,}",
            "[1 2]",
            "tru",
            "\"unterminated",
            "{\"a\":1} trailing",
            "01x",
            "-",
        ] {
            assert!(parse_json(text).is_err(), "{text:?} must not parse");
        }
    }

    // Raw control characters inside strings are rejected on parse (the
    // writer always emits them escaped).
    #[test]
    fn raw_control_chars_rejected() {
        assert!(parse_json("\"a\u{0007}b\"").is_err());
    }

    // Non-ASCII text survives the round trip.
    #[test]
    fn utf8_round_trip() {
        let text = "拼图 — enthusiast™ 日本語";
        let encoded = write_json(&Json::str(text)).expect("writes");
        assert_eq!(
            parse_json(&encoded).expect("parses"),
            Json::Str(text.to_string())
        );
    }

    // M-final review fix: unbounded recursion is gone from BOTH directions.
    // A hostile deeply nested document parses to a clean Err (never a stack
    // overflow), and an over-deep VALUE writes to a clean Err; the boundary
    // itself round-trips exactly.
    #[test]
    fn depth_cap_rejects_deep_nesting_cleanly() {
        let nested_text = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));

        // Parser: exactly MAX_DEPTH levels still parse, one more is a clean
        // error naming the depth.
        let at_cap = parse_json(&nested_text(MAX_DEPTH)).expect("the cap itself parses");
        let over_cap = parse_json(&nested_text(MAX_DEPTH + 1)).expect_err("must not recurse");
        assert!(
            over_cap.message.contains("depth"),
            "expected a depth rejection, got: {over_cap}"
        );
        // The capped value really is MAX_DEPTH levels deep (the innermost
        // level is the empty array).
        let mut depth_of = 0usize;
        let mut walker = &at_cap;
        while let Json::Array(items) = walker {
            depth_of += 1;
            match items.first() {
                Some(inner) => walker = inner,
                None => break,
            }
        }
        assert_eq!(depth_of, MAX_DEPTH);

        // Writer: the same boundary from the value side, with a clean
        // DepthError (not a crash) beyond it.
        let mut deep = Json::int(0);
        for _ in 0..MAX_DEPTH {
            deep = Json::arr(vec![deep]);
        }
        assert!(write_json(&deep).is_ok(), "the cap itself writes");
        let mut deeper = Json::arr(vec![deep]);
        for _ in 0..3 {
            deeper = Json::arr(vec![deeper]);
        }
        let error = write_json(&deeper).expect_err("must not recurse");
        assert_eq!(error.max_depth, MAX_DEPTH);
        assert!(error.to_string().contains("256"));

        // Parse->write identity holds at the cap.
        let written = write_json(&at_cap).expect("writes");
        assert_eq!(parse_json(&written).expect("parses"), at_cap);
        assert_eq!(written, nested_text(MAX_DEPTH));
    }
}
