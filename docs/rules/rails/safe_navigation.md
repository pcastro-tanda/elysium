# Rails/SafeNavigation

Use Ruby's safe navigation operator (`&.`) instead of `try!`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Converts usages of `try!` to `&.`. It can also be configured to convert `try`. It will convert code to use safe navigation if the target Ruby version is set to 2.3+.

```ruby
# bad
foo.try!(:bar)
foo.try!(:bar, baz)
foo.try!(:bar) { |e| e.baz }
foo.try!(&:bar)

# good
foo.try(:bar)
foo&.bar
foo&.bar(baz)
foo&.bar { |e| e.baz }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ConvertTry | false |  | Also convert usages of `try`, not only `try!`. |

## Blind spots

None recorded.
