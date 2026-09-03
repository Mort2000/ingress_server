//! Frozen lexical stripper (D73): blanks `//` and (nested) `/* */`
//! comments plus string/char/raw-string literals from Rust source so
//! token scans see only code. Lexical, not AST — it catches everything
//! an AST would catch for the KEYWORD tokens audited here, and may
//! over-flag pathological code that only LOOKS like a banned token
//! (over-flagging is the safe direction). Lifetimes are kept (they are
//! not literals); raw-identifier `r#` prefixes are stripped so
//! `r#keyword` still reads as the keyword.

/// Returns the source with comment bodies and literal bodies replaced
/// by spaces (quotes kept, newlines kept, offsets preserved).
pub fn strip_source(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        // Line comment: `// ...` to end of line.
        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(b' ');
                i += 1;
            }
            continue;
        }
        // Block comment (nested per the Rust reference).
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let mut depth = 1usize;
            i += 2;
            while i < bytes.len() && depth > 0 {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    if bytes[i] == b'\n' {
                        out.push(b'\n'); // keep line structure
                    }
                    i += 1;
                }
            }
            continue;
        }
        // String-ish literals, with optional b/r/br prefixes.
        match literal_prefix(bytes, i) {
            LiteralArm::Skip(len) => {
                for _ in 0..len {
                    out.push(b' ');
                }
                i += len;
            }
            LiteralArm::String(start_len) => {
                for _ in 0..start_len {
                    out.push(b' ');
                }
                i += start_len;
                out.push(b'"');
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2; // consume the escape pair
                    } else {
                        if bytes[i] == b'\n' {
                            out.push(b'\n');
                        }
                        i += 1;
                    }
                }
                if i < bytes.len() {
                    out.push(b'"');
                    i += 1;
                }
            }
            LiteralArm::RawString => {
                // r#..#"..."#..# — count hashes, scan for the close.
                i += 1; // past `r` (a leading `b` was consumed below)
                let mut hashes = 0usize;
                while i < bytes.len() && bytes[i] == b'#' {
                    hashes += 1;
                    i += 1;
                }
                if bytes.get(i) == Some(&b'"') {
                    i += 1;
                    loop {
                        if i >= bytes.len() {
                            break;
                        }
                        if bytes[i] == b'"' {
                            let mut k = i + 1;
                            let mut h = 0;
                            while h < hashes && bytes.get(k) == Some(&b'#') {
                                k += 1;
                                h += 1;
                            }
                            if h == hashes {
                                i += 1 + hashes;
                                break;
                            }
                        }
                        if bytes[i] == b'\n' {
                            out.push(b'\n');
                        }
                        i += 1;
                    }
                }
                // The whole literal (prefix included) becomes spaces in
                // `out` — nothing code-shaped remains.
                while out.len() < i {
                    out.push(b' ');
                }
                continue;
            }
            LiteralArm::Char(start_len) => {
                for _ in 0..start_len {
                    out.push(b' ');
                }
                i += start_len;
                out.push(b'\'');
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2; // escape pair
                    } else {
                        i += 1;
                    }
                }
                if i < bytes.len() {
                    out.push(b'\'');
                    i += 1;
                }
            }
            LiteralArm::RawIdent => {
                out.push(b' ');
                out.push(b' ');
                i += 2;
            }
            LiteralArm::None => {
                if b == b'\'' && is_char_literal(bytes, i) {
                    out.push(b'\'');
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'\'' {
                        if bytes[i] == b'\\' && i + 1 < bytes.len() {
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    if i < bytes.len() {
                        out.push(b'\'');
                        i += 1;
                    }
                } else {
                    out.push(b);
                    i += 1;
                }
            }
        }
    }
    // Re-pad: `out` is built by index-skipping in some arms, but every
    // arm pushes (or pads below) so lengths stay aligned.
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Debug, PartialEq, Eq)]
enum LiteralArm {
    /// Prefix bytes to blank before the literal proper (b/r/br).
    Skip(usize),
    String(usize),
    RawString,
    Char(usize),
    RawIdent,
    None,
}

/// Classifies the literal starting at `i`, if any.
fn literal_prefix(bytes: &[u8], i: usize) -> LiteralArm {
    let b0 = bytes[i];
    let b1 = bytes.get(i + 1).copied();
    let b2 = bytes.get(i + 2).copied();
    match (b0, b1, b2) {
        (b'b', Some(b'"'), _) => LiteralArm::String(2),
        (b'b', Some(b'\''), _) => LiteralArm::Char(2),
        (b'b', Some(b'r'), Some(b'"' | b'#')) => LiteralArm::Skip(1), // br -> r arm
        (b'r', Some(b'"'), _) => LiteralArm::RawString,
        (b'r', Some(b'#'), Some(b'"')) => LiteralArm::RawString,
        (b'r', Some(b'#'), Some(c)) if is_ident_start(c) => LiteralArm::RawIdent,
        (b'"', _, _) => LiteralArm::String(1),
        _ => LiteralArm::None,
    }
}

fn is_ident_start(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphabetic() || c >= 0x80
}

fn is_ident_continue(c: u8) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

/// `'a'`, `'\n'`, `'\\'`, `'1'`, `' '` are literals; `'a`, `'static`,
/// `'_` are lifetimes.
fn is_char_literal(bytes: &[u8], i: usize) -> bool {
    match bytes.get(i + 1) {
        None | Some(b'\'') => false,
        Some(b'\\') => true, // escape: literal for sure
        Some(c) if !is_ident_continue(*c) => bytes.get(i + 2) == Some(&b'\''),
        Some(_) => {
            let mut j = i + 1;
            while j < bytes.len() && is_ident_continue(bytes[j]) {
                j += 1;
            }
            bytes.get(j) == Some(&b'\'')
        }
    }
}

/// True when `word` occurs in `code` (the STRIPPED source) with
/// identifier boundaries on both sides (`unsafe_code` does not match
/// `unsafe`; `r#unsafe` does, by prefix stripping).
pub fn contains_word(code: &str, word: &str) -> bool {
    let hay = code.as_bytes();
    let needle = word.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_ident_continue(hay[i - 1]);
            let after = i + needle.len();
            let after_ok = after >= hay.len() || !is_ident_continue(hay[after]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Plain substring scan (for path-ish tokens like `std::net`).
pub fn contains_substring(code: &str, needle: &str) -> bool {
    code.contains(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_literals_are_blanked() {
        let src = r##"
// unsafe in a line comment
let a = "unsafe inside a string";
let b = r#"unsafe inside a raw string"#;
/* unsafe in a block /* nested unsafe */ comment */
let c = 'u'; // char literal
let d = '\u{75}'; // escaped char literal (the letter u)
let lit = "quote \" unsafe still string";
"##;
        let code = strip_source(src);
        assert!(!contains_word(&code, "unsafe"), "stripped: {code:?}");
    }

    #[test]
    fn code_tokens_survive_and_raw_idents_flag() {
        let code = strip_source("let x = unsafe { 1 }; let r#unsafe = 2;");
        assert!(contains_word(&code, "unsafe"));
        assert!(contains_word(&code, "let"));
    }

    #[test]
    fn lifetimes_are_not_literals() {
        let code = strip_source("fn f<'a>(x: &'a str) -> &'a str { x }");
        assert!(contains_word(&code, "str"));
        assert!(code.contains("'a"), "lifetimes kept: {code:?}");
    }

    #[test]
    fn word_boundaries_hold() {
        let code = strip_source("let unsafe_code = 1;");
        assert!(!contains_word(&code, "unsafe"));
        assert!(contains_word(&code, "unsafe_code"));
    }
}
