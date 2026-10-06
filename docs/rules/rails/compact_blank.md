# Rails/CompactBlank

Checks if collection can be blank-compacted with `compact_blank`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks if collection can be blank-compacted with `compact_blank`.

It is unsafe by default because false positives may occur in the blank check of block arguments to the receiver object.

```ruby
# bad
collection.reject(&:blank?)
collection.reject { |_k, v| v.blank? }
collection.select(&:present?)
collection.select { |_k, v| v.present? }

# good
collection.compact_blank

# bad
collection.delete_if(&:blank?)
collection.keep_if(&:present?)

# good
collection.compact_blank!
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
