# Performance/RegexpMatch

Use `match?` instead of `Regexp#match`, `String#match`, `Symbol#match`, `Regexp#===`, or `=~` when `MatchData` is not used.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

In Ruby 2.4, `String#match?`, `Regexp#match?`, and `Symbol#match?` have been added. The methods are faster than `match`, because they avoid creating a `MatchData` object or saving backref. So, when `MatchData` is not used, use `match?` instead of `match`.

```ruby
# bad
if x =~ /re/
  do_something
end

# good
if x.match?(/re/)
  do_something
end

# good
if x =~ /re/
  do_something(Regexp.last_match)
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
