# Style/MultilineMethodSignature

Avoid multi-line method signatures.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# good

def foo(bar, baz)
end

# bad

def foo(bar,
        baz)
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
