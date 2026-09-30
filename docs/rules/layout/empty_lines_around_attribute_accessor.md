# Layout/EmptyLinesAroundAttributeAccessor

Keep blank lines around attribute accessors.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for a newline after an attribute accessor or a group of them.
`alias` syntax and `alias_method`, `public`, `protected`, and `private`
methods are allowed by default. These are customizable with
`AllowAliasSyntax` and `AllowedMethods` options.

```ruby
# bad
attr_accessor :foo
def do_something
end

# good
attr_accessor :foo

def do_something
end

# good
attr_accessor :foo
attr_reader :bar
attr_writer :baz
attr :qux

def do_something
end
```

With `AllowAliasSyntax: true` (default):

```ruby
# good
attr_accessor :foo
alias :foo? :foo

def do_something
end
```

With `AllowAliasSyntax: false`:

```ruby
# bad
attr_accessor :foo
alias :foo? :foo

def do_something
end

# good
attr_accessor :foo

alias :foo? :foo

def do_something
end
```

With `AllowedMethods: ['private']`:

```ruby
# good
attr_accessor :foo
private :foo

def do_something
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowAliasSyntax | true |  | Whether an `alias` immediately following an accessor is exempt from the blank-line requirement. |
| AllowedMethods | `alias_method`, `public`, `protected`, `private` |  | Method names that may immediately follow an accessor without a blank line. |

## Blind spots

None recorded.
