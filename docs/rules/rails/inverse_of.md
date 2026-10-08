# Rails/InverseOf

Checks for associations where the inverse cannot be determined automatically.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Looks for has_(one|many) and belongs_to associations where Active Record can't automatically determine the inverse association because of a scope or the options used. Using the blog with order scope example below, traversing the a Blog's association in both directions with `blog.posts.first.blog` would cause the `blog` to be loaded from the database twice.

`:inverse_of` must be manually specified for Active Record to use the associated object in memory, or set to `false` to opt-out. Note that setting `nil` does not stop Active Record from trying to determine the inverse automatically, and is not considered a valid value for this.

```ruby
# bad
class Blog < ApplicationRecord
  has_many :posts, -> { order(published_at: :desc) }
end

# good
class Blog < ApplicationRecord
  has_many(:posts,
           -> { order(published_at: :desc) },
           inverse_of: :blog)
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IgnoreScopes | false |  | Do not require `:inverse_of` for associations with a scope. |

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
