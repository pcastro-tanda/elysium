# Style/OrAssignment

Recommend usage of double pipe equals (||=) where applicable.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for potential usage of the `||=` operator.

```ruby
# bad
name = name ? name : 'Bozhidar'

# bad
name = if name
         name
       else
         'Bozhidar'
       end

# bad
unless name
  name = 'Bozhidar'
end

# bad
name = 'Bozhidar' unless name

# good - set name to 'Bozhidar', only if it's nil or false
name ||= 'Bozhidar'
```

## Options

This rule has no options.

## Blind spots

None recorded.
