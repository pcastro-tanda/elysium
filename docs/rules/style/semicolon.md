# Style/Semicolon

Don't use semicolons to terminate expressions.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for multiple expressions placed on the same line. It also checks for
lines terminated with a semicolon. In idiomatic Ruby, each expression should
be on its own line for readability.

This cop has `AllowAsExpressionSeparator` configuration option. It allows
`;` to separate several expressions on the same line.

```ruby
# bad
foo = 1; bar = 2;
baz = 3;

# good
foo = 1
bar = 2
baz = 3
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowAsExpressionSeparator | false |  | Allows `;` to separate several expressions on the same line. |

## Blind spots

Raw-byte scanning rather than a real token stream: a `;` that is the first
or last *visible* non-whitespace text of its physical line but is actually a
continuation of a token that started on an earlier physical line (a
multi-line, non-heredoc string or a backslash-continued line, e.g. the `;`
right after a multi-line string literal's closing quote) is classified as if
it began its own token there. Not exercised by any known real-world Ruby
style.
