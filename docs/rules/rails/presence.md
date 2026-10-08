# Rails/Presence

Checks code that can be written more easily using `Object#presence` defined by Active Support.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks code that can be written more easily using `Object#presence` defined by Active Support.

```ruby
# bad
a.present? ? a : nil

# bad
!a.present? ? nil : a

# bad
a.blank? ? nil : a

# bad
!a.blank? ? a : nil

# good
a.presence
```

```ruby
# bad
a.present? ? a : b

# bad
!a.present? ? b : a

# bad
a.blank? ? b : a

# bad
!a.blank? ? a : b

# good
a.presence || b
```

```ruby
# bad
a.present? ? a.foo : nil

# bad
!a.present? ? nil : a.foo

# bad
a.blank? ? nil : a.foo

# bad
!a.blank? ? a.foo : nil

# good
a.presence&.foo
```

```ruby
# good
a.present? ? a[1] : nil

# good
a[:key] = value if a.present?

# good
a.present? ? a > 1 : nil

# good
a <= 0 if a.present?

# good
a << "bar" if a.present?
```

## Options

This rule has no options.

## Blind spots

None recorded.
