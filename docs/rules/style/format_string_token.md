# Style/FormatStringToken

Use a consistent style for format string tokens.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for a consistent style for tokens within a format string.

By default, all strings are evaluated. In some cases, this may be undesirable,
as they could be used as arguments to a method that does not consider them to
be tokens, but rather other identifiers or just part of the string.
`AllowedMethods`/`AllowedPatterns` can mark specific methods as always
allowed, avoiding an offense from the cop. By default, there are no allowed
methods.

Additionally, the cop can be made conservative by configuring it with
`Mode: conservative` (default `aggressive`). In this mode, tokens
(regardless of `EnforcedStyle`) are only considered if used in the format
string argument to the methods `printf`, `sprintf`, `format` and `%`.

NOTE: In `aggressive` mode, offenses are registered for all strings
containing tokens, but autocorrection is only applied when the string
appears in a known formatting context (`format`, `sprintf`, `printf`, or
`%`), to prevent false autocorrections for strings that are not actually
format strings.

NOTE: Tokens in the `unannotated` style (eg. `%s`) are always treated as if
configured with `Conservative: true`, to prevent false positives, because
this format is very similar to encoded URLs or Date/Time formatting
strings.

```ruby
# EnforcedStyle: annotated (default)

# bad
format('%{greeting}', greeting: 'Hello')
format('%s', 'Hello')

# good
format('%<greeting>s', greeting: 'Hello')
```

It is allowed to contain unannotated tokens if the number of them is less
than or equal to `MaxUnannotatedPlaceholdersAllowed` (default `1`).

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `annotated` | `annotated`, `template`, `unannotated` | Which format string token style to enforce. |
| MaxUnannotatedPlaceholdersAllowed | 1 |  | Number of unannotated-style tokens allowed in a format string when the enforced style is not `unannotated`. |
| Mode | `aggressive` | `aggressive`, `conservative` | `conservative` only considers strings given to `printf`/`sprintf`/`format`/`%`; `aggressive` (default) considers every string. |
| AllowedMethods | `[]` |  | Method names whose string arguments are always allowed. |
| AllowedPatterns | `[]` |  | Method name regex patterns whose string arguments are always allowed. |

## Blind spots

`AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match) rather than raising a configuration error.

The hand-rolled format-sequence scanner (this rule's replacement for upstream's single `Utils::FormatString::SEQUENCE` regex, since Rust's `regex` crate has no lookaround) does not model `WIDTH`/`PRECISION`'s `#{...}` interpolation alternative; a real interpolation can never reach it (Prism always splits one into its own node before this rule ever sees the literal's raw text), so this only matters for a width/precision component that itself contains an escaped `\#{...}`-looking byte run, which is rejected instead of parsed as a dynamic width/precision.

Faithfully reproducing upstream's own `FormatSequence#precision` (whose capture group excludes the `.`) and `autocorrect_sequence` (which concatenates `flags`/`width`/`precision`/`type` with no separators): converting a named sequence that also carries a precision (e.g. `%<foo>.2f` to `template` style) drops the separating `.` from the correction, matching RuboCop's own behavior for that combination -- unexercised by this cop's own spec, which never autocorrects a precision alongside a name.

RuboCop's `ConfigurableEnforcedStyle` auto-detection (`config_to_allow_offenses`, used only to generate a `.rubocop_todo.yml` entry) is not implemented; it does not affect ordinary offense reporting.
