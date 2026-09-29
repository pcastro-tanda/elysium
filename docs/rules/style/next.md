# Style/Next

Use `next` to skip iteration instead of a condition at the end.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Use `next` to skip iteration instead of a condition at the end.

```ruby
# bad
[1, 2].each do |a|
  if a == 1
    puts a
  end
end

# good
[1, 2].each do |a|
  next unless a == 1
  puts a
end
```

With `EnforcedStyle: always` (default `skip_modifier_ifs`), a modifier `if`
at the end of an iteration is converted too:

```ruby
# bad
[1, 2].each do |a|
  puts a if a == 1
end

# good
[1, 2].each do |a|
  next unless a == 1
  puts a
end
```

With `AllowConsecutiveConditionals: true` (default `false`), a conditional
at the end of an iteration immediately preceded by another conditional at
the same depth is left alone:

```ruby
# good
[1, 2].each do |a|
  if a == 1
    puts a
  end
  if a == 2
    puts a
  end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `skip_modifier_ifs` | `skip_modifier_ifs`, `always` | Whether a modifier `if`/`unless` at the end of an iteration is exempt. |
| MinBodyLength | 3 |  | Minimum number of lines a block-form `if`/`unless` body needs to trigger this cop. |
| AllowConsecutiveConditionals | false |  | Whether a conditional immediately preceded by another conditional at the same depth is exempt. |

## Blind spots

`MinBodyLength`'s upstream validation (must be a positive integer, else the cop raises) is not reproduced; a non-positive value is used as configured instead.
