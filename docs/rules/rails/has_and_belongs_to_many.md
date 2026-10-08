# Rails/HasAndBelongsToMany

Prefer has_many :through to has_and_belongs_to_many.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for the use of the `has_and_belongs_to_many` macro.

```ruby
# bad
has_and_belongs_to_many :ingredients

# good
has_many :ingredients, through: :recipe_ingredients
```

## Options

This rule has no options.

## Blind spots

None recorded.
