# Security/MarshalLoad

Checks for the use of `Marshal` class methods which have potential security issues.

| | |
| --- | --- |
| Department | Security |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for the use of Marshal class methods which have
potential security issues leading to remote code execution when
loading from an untrusted source.

```ruby
# bad
Marshal.load("{}")
Marshal.restore("{}")

# good
Marshal.dump("{}")

# okish - deep copy hack
Marshal.load(Marshal.dump({}))
```

## Options

This rule has no options.

## Blind spots

None recorded.
