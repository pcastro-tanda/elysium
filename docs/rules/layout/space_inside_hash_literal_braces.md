# Layout/SpaceInsideHashLiteralBraces

Checks that braces used for hash literals have or don't have surrounding space depending on configuration. Hash pattern matching is handled in the same way.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: space (default)
# The `space` style enforces that hash literals have surrounding space.

# bad
h = {a: 1, b: 2}
foo = {{ a: 1 } => { b: { c: 2 }}}

# good
h = { a: 1, b: 2 }
foo = { { a: 1 } => { b: { c: 2 } } }
```

```ruby
# EnforcedStyle: no_space
# The `no_space` style enforces that hash literals have no surrounding space.

# bad
h = { a: 1, b: 2 }
foo = {{ a: 1 } => { b: { c: 2 }}}

# good
h = {a: 1, b: 2}
foo = {{a: 1} => {b: {c: 2}}}
```

```ruby
# EnforcedStyle: compact
# The `compact` style normally requires a space inside hash braces, with the
# exception that successive left braces or right braces are collapsed
# together in nested hashes.

# bad
h = { a: { b: 2 } }
foo = { { a: 1 } => { b: { c: 2 } } }

# good
h = { a: { b: 2 }}
foo = {{ a: 1 } => { b: { c: 2 }}}
```

```ruby
# EnforcedStyleForEmptyBraces: no_space (default)
# The `no_space` EnforcedStyleForEmptyBraces style enforces that empty hash
# braces do not contain spaces.

# bad
foo = { }
bar = {    }
baz = {
}

# good
foo = {}
bar = {}
baz = {}
```

```ruby
# EnforcedStyleForEmptyBraces: space
# The `space` EnforcedStyleForEmptyBraces style enforces that empty hash
# braces contain space.

# bad
foo = {}

# good
foo = { }
foo = {    }
foo = {
}
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `space` | `space`, `no_space`, `compact` | Whether hash literal braces require, forbid, or (for `compact`) selectively collapse surrounding space. |
| EnforcedStyleForEmptyBraces | `no_space` | `space`, `no_space` | Whether empty hash literal braces (`{}`) require or forbid a space between them. |

## Blind spots

Scans raw bytes between a hash literal's own opening/closing brace spans in
place of RuboCop's lexer token stream (see the module docs for the mapping);
this reaches every case the cop's `check`/`check_whitespace_only_hash`
reach, but two extremely unlikely inputs diverge from RuboCop's exact `\s`:
`u8::is_ascii_whitespace` does not treat a lone vertical tab (`\v`) as
whitespace, and a hash whose only interior content is itself only reachable
through a byte RuboCop's lexer would have tokenized differently is not
specially handled. `ambiguous_style_detected`/`unexpected_style_detected`
(feeds `--auto-gen-config`) is not ported; it never changes an offense or
its message.
