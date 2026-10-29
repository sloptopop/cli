//! Masking the lines of the commit range before upload (decision D-0915,
//! masked like the snapshot, D-0174). The secret scanner's report covers
//! only the files of `HEAD`, not the lines of older commits, so every line
//! is masked by its shape: a token is a run of ASCII letters, digits and
//! `_ - + / =`; on a line that names a credential (`key`, `secret`,
//! `token`, `passw`, `auth`, …) or holds a documented credential prefix,
//! every quoted token of at least 8 bytes and every bare one of at least
//! 16 holding a letter and a digit is replaced by `x`s of the same length;
//! on any other line, every token of at least 16 bytes holding a letter and
//! a digit; and every token on the lines of a PEM private-key block. A
//! documented prefix (`sk_live_`, `ghp_`, `AKIA`, …) is kept, so the
//! provider stays known, and PEM armour (a run holding `-----`) is never
//! masked. The server's secret backstop scans the range too and refuses
//! anything left in the clear.
//!
//! Standards: OWASP ASVS 5.0 V13.3.1 (secrets never leave the machine);
//! SCF IAC-14, PRI-05; CWE-200.

/// Documented public prefixes of credential formats (Stripe, GitHub,
/// GitLab, Slack, AWS, Google, npm), kept in the clear.
const PREFIXES: [&str; 20] = [
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "rk_test_",
    "pk_live_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxr-",
    "AKIA",
    "ASIA",
    "AIza",
    "npm_",
];

/// Words (in lower case) of a line that names a credential.
const KEYWORDS: [&str; 11] = [
    "access", "api", "auth", "bearer", "cred", "key", "passw", "private", "pwd", "secret", "token",
];

/// The shortest token masked inside quotes on a line that names a
/// credential.
const QUOTED: usize = 8;

/// The shortest token masked otherwise (it must hold a letter and a digit).
const MIXED: usize = 16;

/// Whether `byte` may be part of a token.
const fn token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'/' | b'=')
}

/// `lines` (consecutive lines of one hunk) with their secret-like values
/// masked.
pub(crate) fn mask(lines: &[String]) -> Vec<String> {
    let mut in_key = false;
    lines
        .iter()
        .map(|line| {
            let mut bytes = line.as_bytes().to_vec();
            if in_key {
                if line.contains("-----END") {
                    in_key = false;
                } else {
                    runs(&mut bytes, 0, line.len(), 1, false);
                }
            } else {
                masked_line(&mut bytes);
                in_key = line.contains("-----BEGIN")
                    && line.contains("PRIVATE KEY-----")
                    && !line.contains("-----END");
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
        .collect()
}

/// Masks the tokens of one line outside a key block.
fn masked_line(bytes: &mut [u8]) {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    let named = KEYWORDS.iter().any(|word| text.contains(word))
        || PREFIXES
            .iter()
            .any(|prefix| bytes.windows(prefix.len()).any(|w| w == prefix.as_bytes()));
    let (quoted, mixed) = if named {
        (QUOTED, false)
    } else {
        (MIXED, true)
    };
    let len = bytes.len();
    let mut at = 0;
    let mut bare = 0;
    while let Some(&byte) = bytes.get(at) {
        if matches!(byte, b'\'' | b'"' | b'`') {
            runs(bytes, bare, at, MIXED, true);
            let close = closing(bytes, at + 1, byte);
            runs(bytes, at, close, quoted, mixed);
            at = close + 1;
            bare = at;
        } else {
            at += 1;
        }
    }
    runs(bytes, bare, len, MIXED, true);
}

/// The index of the quote that closes one opened before `from` (or the
/// end of the line).
fn closing(bytes: &[u8], from: usize, quote: u8) -> usize {
    let mut at = from;
    while let Some(&byte) = bytes.get(at) {
        if byte == quote {
            return at;
        }
        at += if byte == b'\\' { 2 } else { 1 };
    }
    bytes.len()
}

/// Masks the token runs of `bytes[from..to]` of at least `min` bytes
/// (with `mixed`, only those holding a letter and a digit; never PEM
/// armour), a documented prefix kept.
fn runs(bytes: &mut [u8], from: usize, to: usize, min: usize, mixed: bool) {
    let segment = bytes.get_mut(from..to).unwrap_or_default();
    for run in segment.split_mut(|byte| !token(*byte)) {
        let letter = run.iter().any(u8::is_ascii_alphabetic);
        let digit = run.iter().any(u8::is_ascii_digit);
        let armour = run.windows(5).any(|window| window == b"-----");
        let kept = PREFIXES
            .iter()
            .filter(|prefix| run.starts_with(prefix.as_bytes()))
            .map(|prefix| prefix.len())
            .max()
            .unwrap_or(0);
        if run.len() >= min && !armour && (!mixed || (letter && digit)) {
            for byte in run.iter_mut().skip(kept) {
                *byte = b'x';
            }
        }
    }
}

#[cfg(test)]
mod tests;
