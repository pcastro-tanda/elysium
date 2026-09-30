# Layout/EmptyLinesAroundExceptionHandlingKeywords

Keeps track of empty lines around exception handling keywords.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# good

begin
  do_something
rescue
  do_something2
else
  do_something3
ensure
  do_something4
end

# good

def foo
  do_something
rescue
  do_something2
end

# bad

begin
  do_something

rescue

  do_something2

else

  do_something3

ensure

  do_something4
end

# bad

def foo
  do_something

rescue

  do_something2
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
