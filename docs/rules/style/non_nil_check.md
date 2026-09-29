# Style/NonNilCheck

Checks for redundant nil checks.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for non-nil checks, which are usually redundant.

With `IncludeSemanticChanges` set to `false` by default, this cop
does not report offenses for `!x.nil?` and does no changes that might
change behavior.
Also `IncludeSemanticChanges` set to `false` with `EnforcedStyle: comparison` of
`Style/NilComparison` cop, this cop does not report offenses for `x != nil` and
does no changes to `!x.nil?` style.

With `IncludeSemanticChanges` set to `true`, this cop reports offenses
for `!x.nil?` and autocorrects that and `x != nil` to solely `x`, which
is *usually* OK, but might change behavior.

```ruby
# bad
if x != nil
end

# good
if x
end

# Non-nil checks are allowed if they are the final nodes of predicate.
# good
def signed_in?
  !current_user.nil?
end
```

With `IncludeSemanticChanges: false` (default):

```ruby
# good
if !x.nil?
end
```

With `IncludeSemanticChanges: true`:

```ruby
# bad
if !x.nil?
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IncludeSemanticChanges | false |  | When `true`, also flags (and auto-corrects, unsafely) `!x.nil?`/`not x.nil?` and `unless x.nil?`, reducing them to bare `x`/`if x`. |

## Blind spots

Upstream's autocorrection for `unless x.nil?` reads `node.receiver.source` unconditionally and would raise on an implicit receiver (`unless nil?`); this port falls back to `self` instead of reproducing the crash.
