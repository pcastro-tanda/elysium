# Rails/Pluck

Prefer `pluck` over `map { ... }`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Enforces the use of `pluck` over `map`.

`pluck` can be used instead of `map` to extract a single key from each element in an enumerable. When called on an Active Record relation, it results in a more efficient query that only selects the necessary key.

The cop ignores `map`/`collect` inside a block whose call has a receiver, since `pluck` there may cause N+1 queries.

This cop is unsafe because the model can use column aliases.

```ruby
# bad
Post.published.map { |post| post[:title] }
[{ a: :b, c: :d }].collect { |el| el[:a] }

# good
Post.published.pluck(:title)
[{ a: :b, c: :d }].pluck(:a)
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
