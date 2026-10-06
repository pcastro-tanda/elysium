# Performance/RedundantMatch

Use `=~` instead of `String#match` or `Regexp#match` in a context where the returned `MatchData` is not needed.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies the use of `Regexp#match` or `String#match`, which returns
`#<MatchData>`/`nil`. The return value of `=~` is an integral index/`nil`
and is more performant.

```ruby
# bad
do_something if str.match(/regex/)
while regex.match('str')
  do_something
end

# good
method(str =~ /regex/)
return value unless regex =~ 'str'
```

## Options

This rule has no options.

## Blind spots

None recorded.
