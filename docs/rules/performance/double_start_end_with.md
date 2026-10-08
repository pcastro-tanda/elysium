# Performance/DoubleStartEndWith

Use `str.{start,end}_with?(x, ..., y, ...)` instead of `str.{start,end}_with?(x, ...) || str.{start,end}_with?(y, ...)`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for consecutive `#start_with?` or `#end_with?` calls. These methods accept multiple arguments, so in some cases like when they are separated by `||`, they can be combined into a single method call.

`IncludeActiveSupportAliases` configuration option is used to check for `starts_with?` and `ends_with?`. These methods are defined by Active Support.

```ruby
# bad
str.start_with?("a") || str.start_with?(Some::CONST)
str.start_with?("a", "b") || str.start_with?("c")
!str.start_with?(foo) && !str.start_with?(bar)
str.end_with?(var1) || str.end_with?(var2)

# good
str.start_with?("a", Some::CONST)
str.start_with?("a", "b", "c")
!str.start_with?(foo, bar)
str.end_with?(var1, var2)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IncludeActiveSupportAliases | false |  | Also check `starts_with?` and `ends_with?` (Active Support). |

## Blind spots

The two calls' receivers are compared by source text rather than RuboCop's structural `Node#==`, so equal receivers written with different formatting (`'a'` vs `"a"`) are treated as different.
