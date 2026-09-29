# Style/StructInheritance

Checks for inheritance from `Struct.new`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for inheritance from `Struct.new`. Inheriting from `Struct.new`
adds a superfluous level in inheritance tree.

Autocorrection is unsafe because it will change the inheritance tree
(e.g. return value of `Module#ancestors`) of the constant.

It is also unsafe because constants that the class body resolves through
its ancestors (e.g. one provided by an included module) fall out of
scope inside the block.

```ruby
# bad
class Person < Struct.new(:first_name, :last_name)
  def age
    42
  end
end

Person.ancestors
# => [Person, #<Class:0x000000010b4e14a0>, Struct, (...)]

# good
Person = Struct.new(:first_name, :last_name) do
  def age
    42
  end
end

Person.ancestors
# => [Person, Struct, (...)]
```

## Options

This rule has no options.

## Blind spots

None recorded.
