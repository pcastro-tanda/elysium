# Lint/UnderscorePrefixedVariableName

Do not use prefix `_` for a variable that is used.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for underscore-prefixed variables that are actually
used.

Since block keyword arguments cannot be arbitrarily named at call
sites, the `AllowKeywordBlockArguments` will allow use of underscore-
prefixed block keyword arguments.

```ruby
# bad
[1, 2, 3].each do |_num|
  do_something(_num)
end

query(:sales) do |_id:, revenue:, cost:|
  {_id: _id, profit: revenue - cost}
end

# good
[1, 2, 3].each do |num|
  do_something(num)
end

[1, 2, 3].each do |_num|
  do_something # not using `_num`
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowKeywordBlockArguments | false |  | Allows use of underscore-prefixed block keyword arguments. |

## Blind spots

None recorded.
