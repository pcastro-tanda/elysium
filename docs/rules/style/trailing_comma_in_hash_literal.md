# Style/TrailingCommaInHashLiteral

Checks for trailing comma in hash literals.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

The configuration options are:

* `consistent_comma`: Requires a comma after the last item of all non-empty,
  multiline hash literals.
* `comma`: Requires a comma after the last item in a hash, but only when
  each item is on its own line.
* `diff_comma`: Requires a comma after the last item in a hash, but only
  when that item is followed by an immediate newline, even if there is an
  inline comment on the same line.
* `no_comma` (default): Does not require a comma after the last item in a
  hash.

```ruby
# EnforcedStyleForMultiline: no_comma (default)

# bad
a = { foo: 1, bar: 2, }

# good
a = {
  foo: 1,
  bar: 2
}
```

```ruby
# EnforcedStyleForMultiline: comma

# bad
a = {
  foo: 1, bar: 2,
  qux: 3
}

# good
a = {
  foo: 1, bar: 2,
  qux: 3,
}
```

```ruby
# EnforcedStyleForMultiline: consistent_comma

# good
a = {
  foo: 1, bar: 2,
  qux: 3,
}

# good
a = {
  foo: 1,
  bar: 2,
}
```

```ruby
# EnforcedStyleForMultiline: diff_comma

# bad
a = { foo: 1, bar: 2,
      baz: 3, qux: 4, }

# good
a = { foo: 1, bar: 2,
      baz: 3, qux: 4 }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleForMultiline | `no_comma` | `comma`, `consistent_comma`, `diff_comma`, `no_comma` | Whether, and when, a multiline hash literal needs a trailing comma. |

## Blind spots

RuboCop's mixin also guards a same-line comma that is actually inside a
trailing comment (`inside_comment?`), comparing the comment's start against
the matched comma's position. That guard is unreachable in practice: the
comma-detection scan requires an unbroken run of whitespace immediately
before the comma, and any interposed `#` comment always breaks that run
first, so the guard's condition can never be reached from a case the scan
already accepts. It is therefore not reproduced here.

Heredoc detection (needed to keep the same-line comma scan from crossing
into a heredoc body) covers a pair's value being a heredoc string/xstring
directly, or reachable through a chain of method calls with no arguments
(recursing into the receiver) or with arguments (recursing into the last
one), matching the mixin's `heredoc?`/`heredoc_send?`. Anything deeper
(e.g. a heredoc nested inside a collection literal value) is not detected
and may cause a false positive comma match inside the heredoc body.
