//! Unit tests for [`super`].

use super::*;

/// `line` masked alone.
fn one(line: &str) -> String {
    mask(&[line.to_owned()]).remove(0)
}

#[test]
fn ordinary_lines_keep_their_words_and_numbers() {
    for line in [
        "expect(a).toBe(2)",
        "expect(total).toEqual(\"two words here\")",
        "\"coverageThreshold\": { \"global\": { \"lines\": 80 } }",
        "\"test\": \"jest --coverage\"",
        "fail_under = 80",
        "a 15charsMixed12 b",
        "\"abcdefghijklmnopqrst\"",
        "",
    ] {
        assert_eq!(one(line), line, "{line:?}");
    }
}

#[test]
fn long_mixed_tokens_are_masked_anywhere() {
    assert_eq!(
        one("id = a1b2c3d4e5f6g7h8"),
        "id = xxxxxxxxxxxxxxxx",
        "bare"
    );
    assert_eq!(
        one("x(\"a1b2c3d4e5f6g7h8\")"),
        "x(\"xxxxxxxxxxxxxxxx\")",
        "quoted"
    );
    assert_eq!(
        one("v = 1234567890123456 and abcdefghijklmnop"),
        "v = 1234567890123456 and abcdefghijklmnop",
        "digits only or letters only"
    );
    assert_eq!(
        one("x = '0123456789abcde'"),
        "x = '0123456789abcde'",
        "15 bytes"
    );
}

#[test]
fn lines_that_name_a_credential_lose_quoted_values() {
    assert_eq!(
        one("const apiKey = 'hunter22'"),
        "const apiKey = 'xxxxxxxx'",
        "8 bytes"
    );
    assert_eq!(
        one("password: \"hunter2\""),
        "password: \"hunter2\"",
        "7 bytes"
    );
    assert_eq!(
        one("token=abcdefghijklmnop"),
        "token=abcdefghijklmnop",
        "bare letters stay"
    );
    assert_eq!(
        one("x = 'sk_live_abcdefgh'"),
        "x = 'sk_live_xxxxxxxx'",
        "a prefix names a credential and is kept"
    );
    assert_eq!(one("y = \"ghp_\""), "y = \"ghp_\"", "a bare prefix stays");
    assert_eq!(
        one("key = 'a' + abcdefghij"),
        "key = 'a' + abcdefghij",
        "a closing quote opens nothing"
    );
    assert_eq!(
        one("secret = \"ab\\\"cdefghij\" + 'k'"),
        "secret = \"ab\\\"xxxxxxxx\" + 'k'",
        "escaped quotes stay inside"
    );
    assert_eq!(
        one("SECRET='unclosed value1234"),
        "SECRET='xxxxxxxx xxxxxxxxx",
        "an unclosed quote runs to the end"
    );
    assert_eq!(
        one("key = '-----BEGIN-----'"),
        "key = '-----BEGIN-----'",
        "armour stays"
    );
}

#[test]
fn private_key_blocks_are_masked_between_their_armour() {
    let lines: Vec<String> = [
        "-----BEGIN RSA PRIVATE KEY-----",
        "MIIB+a/b=",
        "c1",
        "-----END RSA PRIVATE KEY-----",
        "after",
        "-----BEGIN PUBLIC KEY-----",
        "MIIB",
        "-----BEGIN PRIVATE KEY----- -----END PRIVATE KEY-----",
        "plain",
    ]
    .map(str::to_owned)
    .to_vec();
    assert_eq!(
        mask(&lines),
        [
            "-----BEGIN RSA PRIVATE KEY-----",
            "xxxxxxxxx",
            "xx",
            "-----END RSA PRIVATE KEY-----",
            "after",
            "-----BEGIN PUBLIC KEY-----",
            "MIIB",
            "-----BEGIN PRIVATE KEY----- -----END PRIVATE KEY-----",
            "plain",
        ],
        "only the private block"
    );
}
