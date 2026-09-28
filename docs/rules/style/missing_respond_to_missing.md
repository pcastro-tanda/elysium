# Style/MissingRespondToMissing

Checks for the presence of `method_missing` without also defining `respond_to_missing?`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for the presence of `method_missing` without also
defining `respond_to_missing?`.

Not defining `respond_to_missing?` will cause metaprogramming
methods like `respond_to?` to behave unexpectedly:

```ruby
class StringDelegator
  def initialize(string)
    @string = string
  end

  def method_missing(name, *args)
    @string.send(name, *args)
  end
end

delegator = StringDelegator.new("foo")
# Claims to not respond to `upcase`.
delegator.respond_to?(:upcase) # => false
# But you can call it.
delegator.upcase # => FOO
```

```ruby
# bad
def method_missing(name, *args)
  if @delegate.respond_to?(name)
    @delegate.send(name, *args)
  else
    super
  end
end

# good
def respond_to_missing?(name, include_private)
  @delegate.respond_to?(name) || super
end

def method_missing(name, *args)
  if @delegate.respond_to?(name)
    @delegate.send(name, *args)
  else
    super
  end
end
```

## Options

This rule has no options.

## Blind spots

Upstream's `node.parent.parent` ancestor hop does not respect scope
boundaries, so a `respond_to_missing?` nested inside an unrelated nested
class/module within the same enclosing body still counts as implementing
it; this port reproduces that faithfully via `each_descendant` rather than
restricting the search to the immediate class/module body.
