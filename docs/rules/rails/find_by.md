# Rails/FindBy

Prefer find_by over where.first.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies usages of `where.take` and change them to use `find_by` instead.

And `where(...).first` can return different results from `find_by`. (They order records differently, so the "first" record can be different.)

If you also want to detect `where.first`, you can set `IgnoreWhereFirst` to false.

```ruby
# bad
User.where(name: 'Bruce').take

# good
User.find_by(name: 'Bruce')
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IgnoreWhereFirst | true |  | Do not flag `where.first`, which orders differently from `find_by`. |

## Blind spots

None recorded.
