# Performance/EndWith

Use `end_with?` instead of a regex match anchored to the end of a string.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies unnecessary use of a regex where `String#end_with?` would suffice.

This cop has `SafeMultiline` configuration option that `true` by default because `end$` is unsafe as it will behave incompatible with `end_with?` for receiver is multiline string.

```ruby
# bad
'abc'.match?(/bc\Z/)
/bc\Z/.match?('abc')
'abc' =~ /bc\Z/
/bc\Z/ =~ 'abc'
'abc'.match(/bc\Z/)
/bc\Z/.match('abc')

# good
'abc'.end_with?('bc')
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| SafeMultiline | true |  | When true, `$` anchored regexps are not flagged. |

## Blind spots

None recorded.
