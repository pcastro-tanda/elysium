# Lint/EmptyClass

Checks for classes and metaclasses without a body.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for classes and metaclasses without a body.
Such empty classes and metaclasses are typically an oversight or we should provide a comment
to be clearer what we're aiming for.

```ruby
# bad
class Foo
end

class Bar
  class << self
  end
end

class << obj
end

# good
class Foo
  def do_something
    # ... code
  end
end

class Bar
  class << self
    attr_reader :bar
  end
end

class << obj
  attr_reader :bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowComments | false |  | Whether a class or metaclass whose body is only comments counts as empty. |

## Blind spots

None recorded.
