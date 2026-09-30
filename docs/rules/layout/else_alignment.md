# Layout/ElseAlignment

Align elses and elsifs correctly.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

`else` and `elsif` normally line up with the `if`/`unless`/`while`/`until`/
`begin`/`def`/`rescue` keyword they belong to; in an assignment they follow
`Layout/EndAlignment`'s `EnforcedStyleAlignWith` instead.

```ruby
# bad
if something
  code
 else
  code
end

# good
if something
  code
else
  code
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
