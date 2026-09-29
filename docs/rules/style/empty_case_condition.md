# Style/EmptyCaseCondition

Avoid empty condition in case statements.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad:
case
when x == 0
  puts 'x is 0'
when y == 0
  puts 'y is 0'
else
  puts 'neither is 0'
end

# good:
if x == 0
  puts 'x is 0'
elsif y == 0
  puts 'y is 0'
else
  puts 'neither is 0'
end

# good: (the case condition node is not empty)
case n
when 0
  puts 'zero'
when 1
  puts 'one'
else
  puts 'more'
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
