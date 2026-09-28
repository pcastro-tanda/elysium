# Style/AccessModifierDeclarations

Checks style of how access modifiers are used.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Access modifiers should be declared to apply to a group of methods or inline
before each method, depending on configuration. `EnforcedStyle` covers only
method definitions; applications of visibility methods to symbols can be
controlled using `AllowModifiersOnSymbols`, and the visibility of `attr*`
methods using `AllowModifiersOnAttrs`.

@safety
Autocorrection is not safe, because the visibility of dynamically defined
methods can vary depending on the state determined by the group access
modifier.

```ruby
# EnforcedStyle: group (default)
# bad
class Foo
  private def bar; end
  private def baz; end
end

# good
class Foo
  private

  def bar; end
  def baz; end
end
```

```ruby
# EnforcedStyle: inline
# bad
class Foo
  private

  def bar; end
  def baz; end
end

# good
class Foo
  private def bar; end
  private def baz; end
end
```

```ruby
# AllowModifiersOnSymbols: true (default)
# good
class Foo
  private :bar, :baz
  private *%i[qux quux]
end
```

```ruby
# AllowModifiersOnAttrs: true (default)
# good
class Foo
  private attr_accessor :qux
end
```

```ruby
# AllowModifiersOnAliasMethod: true (default)
# good
class Foo
  private alias_method :qux, :foo
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `group` | `inline`, `group` | Whether access modifiers should be inlined before each method, or applied to a group of methods. |
| AllowModifiersOnSymbols | true |  | Allow access modifiers to be used with a symbol (`private :foo`). |
| AllowModifiersOnAttrs | true |  | Allow access modifiers to be used inline with an `attr*` method. |
| AllowModifiersOnAliasMethod | true |  | Allow access modifiers to be used inline with `alias_method`. |

## Blind spots

`ast_with_comments`'s comment association is approximated by a line-based
leading-comment walk (each contiguous comment-only line directly above a
node), rather than a real token-position-aware associator; this only ever
under- or over-includes comments in the deleted range of a correction (never
in the inserted text, which only ever pulls the access-modifier call's own
leading comments), and overlapping deletions are merged, so it cannot change
a correction's output. RuboCop's own auto-style-detection (inferring
`EnforcedStyle` from a mix of correct/incorrect usages when unconfigured) is
not implemented; `EnforcedStyle` must be set explicitly.
