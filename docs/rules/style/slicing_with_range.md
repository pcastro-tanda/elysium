# Style/SlicingWithRange

Checks array slicing is done with redundant, endless, and beginless ranges when suitable.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks that arrays are not sliced with the redundant `ary[0..-1]`, replacing it with `ary`, and ensures arrays are sliced with endless ranges instead of `ary[start..-1]` on Ruby 2.6+, and with beginless ranges instead of `ary[nil..end]` on Ruby 2.7+.

This cop is unsafe because `x..-1` and `x..` are only guaranteed to be equivalent for `Array#[]`, `String#[]`, and the cop cannot determine what class the receiver is.

## Options

This rule has no options.

## Blind spots

Upstream's `minimum_target_ruby_version 2.6` (disabling the whole cop pre-2.6) is not ported: the one spec scenario for it (`ary[1..-1]` reporting no offense on Ruby 2.5) is itself marked `unsupported_on: :prism`, matching `layout/heredoc_indentation.rs`'s precedent.
