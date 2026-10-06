# Rails/Pick

Prefer `pick` over `pluck(...).first`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Enforces the use of `pick` over `pluck(...).first`.

Using `pluck` followed by `first` creates an intermediate array, which `pick` avoids. When called on an Active Record relation, `pick` adds a limit to the query so that only one value is fetched from the database.

Note that when `pick` is added to a relation with an existing limit, it causes a subquery to be added. In most cases this is undesirable, and care should be taken while resolving this violation.

This cop is unsafe because `pluck` is defined on both `ActiveRecord::Relation` and `Enumerable`, whereas `pick` is only defined on `ActiveRecord::Relation` in Rails 6.0. This was addressed in Rails 6.1 via rails/rails#38760, at which point the cop is safe.

```ruby
# bad
Model.pluck(:a).first
[{ a: :b, c: :d }].pluck(:a, :b).first

# good
Model.pick(:a)
[{ a: :b, c: :d }].pick(:a, :b)
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
