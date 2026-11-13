# cache-directive-parser

Parses an HTTP `Cache-Control` header value into structured directives with parameters and flags.

```rust
use cache_directive_parser::{parse, Directive};

fn main() {
    let directives: Vec<Directive> = parse("max-age=60, no-store, private=\"Set-Cookie\"");
    assert_eq!(directives.len(), 3);
    assert_eq!(directives[0].name, "max-age");
    assert_eq!(directives[0].value.as_deref(), Some("60"));
    assert!(directives[1].is_flag());
    assert_eq!(directives[2].value.as_deref(), Some("Set-Cookie"));
}
```

## Why this exists

HTTP caches and proxies often need to inspect `Cache-Control` without pulling in a
full HTTP stack. The header is deceptively simple: commas separate directives, but
a comma inside a quoted string is part of a value. This crate parses the syntax and
leaves semantics alone. It does not reject unknown directives, and it does not
validate numeric arguments such as `max-age`. That keeps the parser small and
predictable for tools that must forward or log directives they do not understand.

The trade-off is deliberate: a caller that needs to enforce a maximum age must
parse the value itself. The parser only guarantees that names are lowercased and
quoted values are unquoted.

## Edge cases

Malformed input is parsed on a best-effort basis. An unterminated quoted value is
treated as if the closing quote were at the end of the input; the value is still
returned. Empty directives caused by stray commas are dropped. An empty input
produces an empty vector.
