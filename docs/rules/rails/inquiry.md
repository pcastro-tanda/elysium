# Rails/Inquiry

Prefer Ruby's comparison operators over Active Support's `Array#inquiry` and `String#inquiry`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks that Active Support's `inquiry` method is not used.

```ruby
# bad - String#inquiry
ruby = 'two'.inquiry
ruby.two?

# good
ruby = 'two'
ruby == 'two'

# bad - Array#inquiry
pets = %w(cat dog).inquiry
pets.gopher?

# good
pets = %w(cat dog)
pets.include? 'cat'
```

## Options

This rule has no options.

## Blind spots

None recorded.
