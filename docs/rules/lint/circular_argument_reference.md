# Lint/CircularArgumentReference

Checks for circular argument references in optional keyword arguments and optional ordinal arguments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for circular argument references in optional keyword
arguments and optional ordinal arguments.

NOTE: This syntax was made invalid on Ruby 2.7 - Ruby 3.3 but is allowed
again since Ruby 3.4.

```ruby
# bad

def bake(pie: pie)
  pie.heat_up
end

# good

def bake(pie:)
  pie.refrigerate
end

# good

def bake(pie: self.pie)
  pie.feed_to(user)
end

# bad

def cook(dry_ingredients = dry_ingredients)
  dry_ingredients.reduce(&:+)
end

# good

def cook(dry_ingredients = self.dry_ingredients)
  dry_ingredients.combine
end

# bad

def foo(pie = pie = pie)
  pie.heat_up
end

# good

def foo(pie)
  pie.heat_up
end

# bad

def foo(pie = cake = pie)
  [pie, cake].each(&:heat_up)
end

# good

def foo(cake = pie)
  [pie, cake].each(&:heat_up)
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
