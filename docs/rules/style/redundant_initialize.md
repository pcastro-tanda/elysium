# Style/RedundantInitialize

Checks for redundant `initialize` methods.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

An initializer is redundant if it does not do anything, or if it only
calls `super` with the same arguments given to it. If the initializer takes
an argument that accepts multiple values (`restarg`, `kwrestarg`, etc.) it
will not register an offense, because it allows the initializer to take a different
number of arguments as its superclass potentially does.

NOTE: If an initializer takes any arguments and has an empty body, RuboCop
assumes it to *not* be redundant. This is to prevent potential `ArgumentError`.

NOTE: If an initializer argument has a default value, RuboCop assumes it
to *not* be redundant.

NOTE: Empty initializers are registered as offenses, but it is possible
to purposely create an empty `initialize` method to override a superclass's
initializer.

```ruby
# bad
def initialize
end

# bad
def initialize
  super
end

# bad
def initialize(a, b)
  super
end

# bad
def initialize(a, b)
  super(a, b)
end

# good
def initialize
  do_something
end

# good
def initialize
  do_something
  super
end

# good (different number of parameters)
def initialize(a, b)
  super(a)
end

# good (default value)
def initialize(a, b = 5)
  super
end

# good (default value)
def initialize(a, b: 5)
  super
end

# good (changes the parameter requirements)
def initialize(_)
end

# good (changes the parameter requirements)
def initialize(*)
end

# good (changes the parameter requirements)
def initialize(**)
end

# good (changes the parameter requirements)
def initialize(...)
end
```

With `AllowComments: true` (the default), a comment anywhere in the method
body is enough to keep it from being flagged.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowComments | true |  | Allow comments in the method body to prevent the method from being removed. |

## Blind spots

`comments_contain_disables?` (an in-body `# rubocop:disable` comment still
counting against `AllowComments`) is not replicated: any comment anywhere
in the method's span keeps it from being flagged, regardless of whether
that comment is itself a disable directive.
