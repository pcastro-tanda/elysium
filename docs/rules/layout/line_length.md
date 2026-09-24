# Layout/LineLength

Checks the length of lines in the source code.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The maximum length is configurable. The tab size is configured in the
`IndentationWidth` of `Layout/IndentationStyle`. A shebang line is ignored
by default.

This cop has some autocorrection capabilities. It can programmatically
shorten certain long lines by inserting line breaks into expressions that
can be safely split across lines -- arrays, hashes, method calls with
argument lists, blocks, and (with `SplitStrings`) string literals.

```ruby
# bad
{foo: "0000000000", bar: "0000000000", baz: "0000000000"}

# good
{foo: "0000000000",
bar: "0000000000", baz: "0000000000"}
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 120 |  | Maximum line length in characters. |
| AllowHeredoc | true |  | Allow long lines inside heredocs (or a list of exempt delimiters). |
| AllowURI | true |  | Allow long lines that consist mostly of a URI. |
| AllowQualifiedName | true |  | Allow long lines that consist mostly of a qualified name (`A::B::C`). |
| URISchemes | `http`, `https` |  | URI schemes considered by `AllowURI`. |
| AllowRBSInlineAnnotation | false |  | Allow long lines that are RBS::Inline annotations. |
| AllowCopDirectives | true |  | Allow long lines that consist mostly of a `# rubocop:` directive. |
| AllowedPatterns | `[]` |  | Lines matching any of these regular expressions are ignored. |
| SplitStrings | false |  | Split long string literals with a continuation when autocorrecting. |
| AutoCorrect | true |  | Whether this cop may autocorrect offenses. |

## Blind spots

Autocorrection ports RuboCop's `CheckLineBreakable` mixin without its
ancestor-chain heuristics (`contained_by_breakable_collection_on_same_line?`
and `contained_by_multiline_collection_that_could_be_broken_up?`): a nested
breakable collection/call sharing a line with an already-claimed outer one is
still usually suppressed (the outer claims the line first in traversal
order), but a few deeply nested or already-partially-broken-up cases may
pick a different (or no) breakable point than RuboCop. `AllowURI`'s URI
matcher approximates RFC 2396 by excluding common non-URI delimiter
characters (quotes, angle brackets, backslash, braces, `|`, `^`, brackets)
rather than parsing the real grammar, so a URI containing one of the rarer
valid-but-unusual characters that delimiter excludes may be split or
truncated where RuboCop's `URI.parse`-validated match would not be.
`AllowedPatterns` entries that use Ruby-only regex
syntax (Oniguruma property names, possessive quantifiers) fail to compile
and are silently skipped (the line is then linted normally). Offense
detection itself (the `Max`/`AllowHeredoc`/`AllowURI`/`AllowQualifiedName`/`AllowCopDirectives`/`AllowRBSInlineAnnotation`/`AllowedPatterns` line checks)
is a complete port.
