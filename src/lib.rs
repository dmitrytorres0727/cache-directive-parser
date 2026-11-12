//! A small parser for HTTP `Cache-Control` header values.
//!
//! The parser is intentionally permissive: it accepts the field value as a byte
//! string and returns a list of directives. Directive names are lowercased so
//! that callers can compare them without worrying about case. A directive is
//! represented as a name plus an optional argument string. Quoted arguments
//! are unquoted and unescaped according to RFC 9111's quoted-string rules.
//!
//! The design trade-off is that this crate does not attempt to validate the
//! semantic meaning of directives. For example, `max-age=abc` is returned as
//! `Directive { name: "max-age", value: Some("abc") }` rather than rejected.
//! Applications that care about numeric ranges or known directive names should
//! interpret the parsed result themselves. Keeping the parser syntax-only makes
//! it useful for proxies, caches, and test tools that need to inspect unknown
//! extensions.

/// A single directive from a `Cache-Control` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// The directive name, lowercased.
    pub name: String,
    /// The directive argument, if one was present.
    ///
    /// For a flag such as `no-store` this is `None`. For `max-age=60` it is
    /// `Some("60")`. Quoted arguments are unquoted and unescaped.
    pub value: Option<String>,
}

impl Directive {
    /// Returns true if the directive has no argument.
    pub fn is_flag(&self) -> bool {
        self.value.is_none()
    }
}

/// Parse a `Cache-Control` header field value into directives.
///
/// The input is the raw field value without the header name. Commas separate
/// directives at the top level; commas inside quoted strings do not. Whitespace
/// around names and values is ignored. An empty input yields an empty vector.
///
/// Malformed input is handled by producing the best-effort directive that can
/// be recovered. For example, an unterminated quoted value is treated as if it
/// were terminated at the end of the input. This is deliberate: HTTP field
/// values are sometimes truncated by intermediaries, and a parser that fails
/// hard on such input is less useful than one that reports what it can.
pub fn parse(input: &str) -> Vec<Directive> {
    let bytes = input.as_bytes();
    let mut directives = Vec::new();
    let mut i = 0;

    // Skip leading whitespace and commas.
    while i < bytes.len() {
        let b = bytes[i];
        if b == b',' || b == b' ' || b == b'\t' {
            i += 1;
        } else {
            break;
        }
    }

    while i < bytes.len() {
        // Parse a directive name. A name is a token: any visible ASCII
        // character except separators. We stop at '=', ',', whitespace, or
        // end of input.
        let name_start = i;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'=' || b == b',' || b == b' ' || b == b'\t' {
                break;
            }
            i += 1;
        }
        let name = input[name_start..i].to_ascii_lowercase();

        // Skip optional whitespace before '='.
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }

        let mut value = None;
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1; // consume '='
            // Skip optional whitespace after '='.
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }

            if i < bytes.len() && bytes[i] == b'"' {
                // Quoted string: find the closing quote, handling backslash
                // escapes. If the quote is missing, take the rest of the input.
                i += 1; // consume opening quote
                let mut s = String::new();
                while i < bytes.len() {
                    let b = bytes[i];
                    if b == b'\\' && i + 1 < bytes.len() {
                        // Backslash escapes the next character.
                        i += 1;
                        s.push(bytes[i] as char);
                        i += 1;
                    } else if b == b'"' {
                        i += 1; // consume closing quote
                        break;
                    } else {
                        s.push(b as char);
                        i += 1;
                    }
                }
                value = Some(s);
            } else {
                // Unquoted value: read until comma or end of input.
                let value_start = i;
                while i < bytes.len() && bytes[i] != b',' {
                    i += 1;
                }
                let raw = input[value_start..i].trim_end_matches(|c: char| c == ' ' || c == '\t');
                value = Some(raw.to_string());
            }
        }

        // Skip trailing whitespace before the next comma.
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }

        // If the next character is a comma, consume it and continue.
        if i < bytes.len() && bytes[i] == b',' {
            i += 1;
        }

        // Skip whitespace and commas before the next directive.
        while i < bytes.len() {
            let b = bytes[i];
            if b == b',' || b == b' ' || b == b'\t' {
                i += 1;
            } else {
                break;
            }
        }

        // Only add a directive if we actually read a name. This avoids
        // producing empty directives for trailing commas.
        if !name.is_empty() {
            directives.push(Directive { name, value });
        }
    }

    directives
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(name: &str, value: Option<&str>) -> Directive {
        Directive {
            name: name.to_string(),
            value: value.map(|s| s.to_string()),
        }
    }

    #[test]
    fn parses_simple_flags() {
        assert_eq!(
            parse("no-store, no-cache"),
            vec![d("no-store", None), d("no-cache", None)]
        );
    }

    #[test]
    fn parses_values() {
        assert_eq!(
            parse("max-age=60, s-maxage=120"),
            vec![d("max-age", Some("60")), d("s-maxage", Some("120"))]
        );
    }

    #[test]
    fn lowercases_names() {
        assert_eq!(parse("No-Store"), vec![d("no-store", None)]);
    }

    #[test]
    fn ignores_whitespace_around_equals() {
        assert_eq!(parse("max-age = 60"), vec![d("max-age", Some("60"))]);
    }

    #[test]
    fn unquotes_quoted_values() {
        assert_eq!(
            parse("private=\"Set-Cookie\""),
            vec![d("private", Some("Set-Cookie"))]
        );
    }

    #[test]
    fn handles_escaped_quotes() {
        assert_eq!(
            parse("foo=\"a\\\"b\""),
            vec![d("foo", Some("a\"b"))]
        );
    }

    #[test]
    fn comma_inside_quotes_is_not_a_separator() {
        assert_eq!(
            parse("foo=\"a,b\", bar"),
            vec![d("foo", Some("a,b")), d("bar", None)]
        );
    }

    #[test]
    fn empty_input_gives_empty_vec() {
        assert_eq!(parse(""), Vec::<Directive>::new());
        assert_eq!(parse("   "), Vec::<Directive>::new());
        assert_eq!(parse(",,,"), Vec::<Directive>::new());
    }

    #[test]
    fn trailing_comma_is_ignored() {
        assert_eq!(parse("no-store,"), vec![d("no-store", None)]);
    }

    #[test]
    fn unterminated_quote_takes_rest() {
        assert_eq!(
            parse("foo=\"bar"),
            vec![d("foo", Some("bar"))]
        );
    }

    #[test]
    fn empty_value_is_preserved() {
        assert_eq!(parse("foo="), vec![d("foo", Some(""))]);
    }

    #[test]
    fn is_flag_works() {
        assert!(d("no-store", None).is_flag());
        assert!(!d("max-age", Some("60")).is_flag());
    }
}
