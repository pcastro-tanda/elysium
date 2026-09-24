# Lint/RedundantCopDisableDirective

Detects instances of rubocop:disable comments that can be removed.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | nursery |

Detects instances of `rubocop:disable` (and `rubocop:todo`) comments that can be removed without causing any offenses to be reported. It waits until every other rule has run and flags any disable directive whose covered cop(s), department(s), or `all` never suppressed a real offense in the range it covers.

```ruby
# bad
# rubocop:disable Layout/LineLength
x += 1
# rubocop:enable Layout/LineLength

# good
x += 1
```

## Options

This rule has no options.

## Blind spots

`each_already_disabled` (RuboCop's "this cop was already disabled by an earlier, still-open directive" duplicate check, which flags a re-disable as redundant unconditionally, even over a real offense in its range) is replicated for the common case a directive names both a department and one of its own members (`# rubocop:disable Metrics, Metrics/ClassLength`), by preserving that member's duplicate occurrence instead of collapsing it (see `expand_directive`); a duplicate re-disable of the exact same cop split across two *separate* directives with no `enable` between them still falls back to the ordinary per-range offense check, under-reporting (false negative, never false positive) the rarer case where the second directive's range has a genuine offense of its own. `DirectiveComment#malformed?`/`push`/`pop` directives are out of `ruby_directives`' scope (see its module docs) and are not handled here either. A directive's trailing free text after the cop list (`# rubocop:disable Foo -- reason`) is dropped along with the rest of the comment on a whole-comment removal instead of being preserved as `# -- reason`. Ambiguous bare cop names (matching more than one department) are left unresolved (a false negative) instead of raising, since there is no way to surface RuboCop's `AmbiguousCopName` configuration error here.
