# Rails/Present

Enforces use of `present?`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for code that can be written with simpler conditionals using `Object#present?` defined by Active Support.

Interaction with `Style/UnlessElse`: the configuration of `NotBlank` will not produce an offense in the context of `unless else` if `Style/UnlessElse` is enabled. This is to prevent interference between the autocorrection of the two cops.

```ruby
# bad
!foo.nil? && !foo.empty?

# bad
foo != nil && !foo.empty?

# bad
!foo.blank?

# bad
not foo.blank?

# bad
something unless foo.blank?

# good
foo.present?

# good
something if foo.present?
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| NotNilAndNotEmpty | true |  | Convert usages of `!nil? && !empty?` to `present?`. |
| NotBlank | true |  | Convert usages of `!blank?` to `present?`. |
| UnlessBlank | true |  | Convert usages of `unless blank?` to `if present?`. |

## Blind spots

None recorded.
