# Lint/DeprecatedOpenSSLConstant

Don't use algorithm constants for `OpenSSL::Cipher` and `OpenSSL::Digest`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Algorithmic constants for `OpenSSL::Cipher` and `OpenSSL::Digest`
deprecated since OpenSSL version 2.2.0. Prefer passing a string
instead.

```ruby
# bad
OpenSSL::Cipher::AES.new(128, :GCM)

# good
OpenSSL::Cipher.new('aes-128-gcm')

# bad
OpenSSL::Digest::SHA256.new

# good
OpenSSL::Digest.new('SHA256')

# bad
OpenSSL::Digest::SHA256.digest('foo')

# good
OpenSSL::Digest.digest('SHA256', 'foo')
```

## Options

This rule has no options.

## Blind spots

The argument skip-list (`arg.variable? || arg.call_type? || arg.const_type?`)
relies on Prism's own parse-time local-variable tracking to tell a bare
identifier read (`lvar`) apart from a zero-arg method call, exactly like
whitequark's parser does upstream; no discrepancy is expected.
