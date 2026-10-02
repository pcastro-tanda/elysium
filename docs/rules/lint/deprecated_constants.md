# Lint/DeprecatedConstants

Checks for deprecated constants.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for deprecated constants.

It has `DeprecatedConstants` config. If there is an alternative method, you can set alternative value as `Alternative`. And you can set the deprecated version as `DeprecatedVersion`. These options can be omitted if they are not needed.

By default, `NIL`, `TRUE`, `FALSE`, `Net::HTTPServerException`, `Random::DEFAULT`, `Struct::Group`, and `Struct::Passwd` are configured.

```ruby
# bad
NIL
TRUE
FALSE
Net::HTTPServerException
Random::DEFAULT # Return value of Ruby 2 is `Random` instance, Ruby 3.0 is `Random` class.
Struct::Group
Struct::Passwd

# good
nil
true
false
Net::HTTPClientException
Random.new # `::DEFAULT` has been deprecated in Ruby 3, `.new` is compatible with Ruby 2.
Etc::Group
Etc::Passwd
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| DeprecatedConstants | `nil` |  | Deprecated constants mapped to an optional `Alternative` and `DeprecatedVersion`. Defaults: NIL, TRUE, FALSE, Net::HTTPServerException, Random::DEFAULT, Struct::Group, Struct::Passwd. |

## Blind spots

None recorded.
