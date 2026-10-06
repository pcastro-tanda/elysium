# Rails/Blank

Enforces use of `blank?`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for code that can be written with simpler conditionals using `Object#blank?` defined by Active Support.

Interaction with `Style/UnlessElse`: the configuration of `NotPresent` will not produce an offense in the context of `unless else` if `Style/UnlessElse` is enabled. This is to prevent interference between the autocorrection of the two cops.

This cop is unsafe autocorrection, because `' '.empty?` returns false, but `' '.blank?` returns true. Therefore, autocorrection is not compatible if the receiver is a non-empty blank string, tab, or newline meta characters.

```ruby
# bad
foo.nil? || foo.empty?
foo == nil || foo.empty?
!foo.present?
something unless foo.present?

# good
foo.blank?
something if foo.blank?

def blank?
  !present?
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| NilOrEmpty | true |  | Convert usages of `nil? || empty?` to `blank?`. |
| NotPresent | true |  | Convert usages of `!present?` to `blank?`. |
| UnlessPresent | true |  | Convert usages of `unless present?` to `if blank?`. |

## Blind spots

Receiver equality in `nil? || empty?` is compared by source text.
