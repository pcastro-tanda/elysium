# Style/SignalException

Checks for proper usage of fail and raise.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of `fail` and `raise`.

* `only_raise` (default) - enforces the sole use of `raise`.
* `only_fail` - enforces the sole use of `fail`.
* `semantic` - uses `fail` to signal an exception, then uses `raise` to
  trigger an offense after it has been rescued.

```ruby
# EnforcedStyle: only_raise (default)
# bad
begin
  fail
rescue Exception
  # handle it
end

# good
begin
  raise
rescue Exception
  # handle it
end

# EnforcedStyle: semantic
# bad
begin
  raise
rescue Exception
  # handle it
end

def watch_out
  # Error thrown
rescue Exception
  fail
end

# good
begin
  fail
rescue Exception
  # handle it
end

def watch_out
  fail
rescue Exception
  raise 'Preferably with descriptive message'
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `only_raise` | `only_raise`, `only_fail`, `semantic` | Whether `fail`, `raise`, or a `semantic` split between them is enforced. |

## Blind spots

`custom_fail_defined?` only widens the `only_raise` exemption; it is not
consulted for `only_fail`/`semantic`, matching upstream.
