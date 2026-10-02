# Style/SelectByRegexp

Prefer `grep`/`grep_v` to `select`/`reject`/`find_all`/`filter` with a regexp match.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Looks for places where a subset of an Enumerable (array, range, set, etc.; see note below) is calculated based on a `Regexp` match, and suggests `grep` or `grep_v` instead.

NOTE: Hashes do not behave as you may expect with `grep`, which means that `hash.grep` is not equivalent to `hash.select`. Although RuboCop is limited by static analysis, this cop attempts to avoid registering an offense when the receiver is a hash (hash literal, `Hash.new`, `Hash#[]`, or `to_h`/`to_hash`).

NOTE: `grep` and `grep_v` were optimized when used without a block in Ruby 3.0, but may be slower in previous versions.

Autocorrection is marked as unsafe because `MatchData` will not be created by `grep`, but may have previously been relied upon after the `match?` or `=~` call. Additionally, the cop cannot guarantee that the receiver of `select` or `reject` is actually an array by static analysis, so the correction may not be actually equivalent.

## Options

This rule has no options.

## Blind spots

None recorded.
