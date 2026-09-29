# Style/WhenThen

Use when x then ... for one-line cases.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for `when;` uses in `case` expressions.

```ruby
# bad
case foo
when 1; 'baz'
when 2; 'bar'
end

# good
case foo
when 1 then 'baz'
when 2 then 'bar'
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
