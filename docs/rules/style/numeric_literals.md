# Style/NumericLiterals

Checks for big numeric literals without `_` between groups of digits in them.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Additional allowed patterns can be added by adding regexps to the
`AllowedPatterns` configuration. All regexps are treated as anchored even if
the patterns do not contain anchors (so `\d{4}_\d{4}` will allow
`1234_5678` but not `1234_5678_9012`).

NOTE: Even if `AllowedPatterns` are given, autocorrection will only correct
to the standard pattern of an `_` every 3 digits.

```ruby
# bad
1000000
1_00_000
1_0000

# good
1_000_000
1000
```

```ruby
# Strict: false (default)

# good
10_000_00 # typical representation of $10,000 in cents
```

```ruby
# Strict: true

# bad
10_000_00 # typical representation of $10,000 in cents
```

```ruby
# AllowedNumbers: [3000]

# good
3000 # You can specify allowed numbers. (e.g. port number)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| MinDigits | 5 |  | The minimum number of (undelimited) digits that triggers an offense. |
| Strict | false |  | Also flag a trailing 1-2 digit group at the end of an otherwise delimited literal (e.g. `10_000_00`). |
| AllowedNumbers | `[]` |  | Specific numbers (compared as written, digits only) exempted from this cop. |
| AllowedPatterns | `[]` |  | Regular expressions (anchored to the whole digit run) exempted from this cop. |

## Blind spots

Non-decimal literals (`0b`/`0o`/`0`/`0x` prefixes) and any float whose integer
part is a bare `0` are skipped by matching RuboCop's own `int.start_with?('0')`
guard verbatim, rather than checking the literal's actual base; this exactly
mirrors upstream's documented limitation (see the cop's `on_int` TODO) so it
is not a gap relative to RuboCop, but it does mean a huge `0.123456789` never
offends, same as real RuboCop. RuboCop's `case/when` distinguishes an
undelimited run from a misdelimited one only to steer its `--auto-gen-config`
suggestion (`MinDigits`/disabling the cop); since both arms always report the
same offense and fix, this port collapses them into one `offends` check and
never emits `--auto-gen-config` metadata (out of scope for a linter/fixer).
A literal that is the sole receiver of a bare unary `-`/`+` call (matching
RuboCop's real-parser sign fusion across whitespace/newlines) is widened to
the call's own span; a receiver reached through anything else (parentheses,
a further method chain, an explicit `.-@` call) is treated as an ordinary
literal instead of being folded in, which can only under- rather than
over-report relative to RuboCop's parser-level fusion.
