# Style/StringLiterals

Checks if uses of quotes match the configured preference.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

```ruby
# EnforcedStyle: single_quotes (default)

# bad
"No special symbols"
"No string interpolation"
"Just text"

# good
'No special symbols'
'No string interpolation'
'Just text'
"Wait! What's #{this}!"
```

```ruby
# EnforcedStyle: double_quotes

# bad
'Just some text'
'No special chars or interpolation'

# good
"Just some text"
"No special chars or interpolation"
"Every string in #{project} uses double_quotes"
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `single_quotes` | `single_quotes`, `double_quotes` | Preferred string literal quote style. |
| ConsistentQuotesInMultiline | false |  | Strings spanning multiple lines using `\` for continuation must use the same type of quotes on each line. |

## Blind spots

Ports whitequark's parser quirk that a string literal spanning multiple
*physical* lines with real embedded newlines (not a heredoc, not a `\n`
escape) is lexed as a `dstr` with one unquoted `str` child per line: by
default (`ConsistentQuotesInMultiline: false`) such a literal is never
checked at all, and even with the option enabled it is judged as a whole
(equivalent to running the single-string check over its full source) but
never autocorrected, matching `StringLiteralCorrector`'s `return if
node.dstr_type?`. Backslash-continued string concatenation (`'a' \\\n'b'`)
is likewise only ever reported, never fixed, matching RuboCop. Strings
nested inside `#{...}` interpolation of another string/symbol/regexp are
never judged individually, matching `inside_interpolation?`; this is
tracked with nesting counters rather than true ancestor walks, so it does
not distinguish further by container kind beyond string/symbol/regexp vs.
everything else (RuboCop itself does not either).
