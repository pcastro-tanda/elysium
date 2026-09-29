# Style/CaseEquality

Avoid explicit use of the case equality operator (`===`).

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The `===` operator has different behavior depending on the receiver and its use outside of `case`/`when` is confusing. Prefer more explicit alternatives like `is_a?`, `include?`, or `match?`.

If `AllowOnConstant` is enabled, the cop ignores violations when the receiver of the case equality operator is a constant. If `AllowOnSelfClass` is enabled, the cop ignores violations when the receiver is `self.class`.

Regexp case equality (`/regexp/ === var`) is always allowed, since rewriting it to `/regexp/.match?(var)` would need to account for `Regexp.last_match?`, `$~`, `$1`, etc.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowOnConstant | false |  | Whether the case equality operator is allowed when its receiver is a constant. |
| AllowOnSelfClass | false |  | Whether the case equality operator is allowed when its receiver is `self.class`. |

## Blind spots

None recorded.
