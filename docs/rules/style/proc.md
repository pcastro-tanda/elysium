# Style/Proc

Use proc instead of Proc.new.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of `Proc.new` where `Kernel#proc`
would be more appropriate. `proc` is the shorter and
more idiomatic way to create procs in Ruby.

```ruby
# bad
p = Proc.new { |n| puts n }

# good
p = proc { |n| puts n }
```

## Options

This rule has no options.

## Blind spots

None recorded.
