# Lint/UselessRescue

Checks for useless `rescue`s.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for useless ``rescue``s, which only reraise rescued exceptions.

```ruby
# bad
def foo
  do_something
rescue
  raise
end

# bad
def foo
  do_something
rescue => e
  raise # or 'raise e', or 'raise $!', or 'raise $ERROR_INFO'
end

# good
def foo
  do_something
rescue
  do_cleanup
  raise
end

# bad (latest rescue)
def foo
  do_something
rescue ArgumentError
  # noop
rescue
  raise
end

# good (not the latest rescue)
def foo
  do_something
rescue ArgumentError
  raise
rescue
  # noop
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
