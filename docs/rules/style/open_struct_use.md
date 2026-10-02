# Style/OpenStructUse

Avoid using OpenStruct. As of Ruby 3.0, use is officially discouraged due to performance, version compatibility, and potential security issues.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

# @safety
Note that this cop may flag false positives; for instance, the following
legal use of a hand-rolled `OpenStruct` type would be considered an
offense:

```ruby
module MyNamespace
  class OpenStruct # not the OpenStruct we're looking for
  end

  def new_struct
    OpenStruct.new # resolves to MyNamespace::OpenStruct
  end
end
```

```ruby
# bad
point = OpenStruct.new(x: 0, y: 1)

# good
Point = Struct.new(:x, :y)
point = Point.new(0, 1)

# also good
point = { x: 0, y: 1 }
```

## Options

This rule has no options.

## Blind spots

None recorded.
