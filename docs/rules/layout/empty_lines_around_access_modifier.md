# Layout/EmptyLinesAroundAccessModifier

Keep blank lines around access modifiers.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: around (default)

# bad
class Foo
  def bar; end
  private
  def baz; end
end

# good
class Foo
  def bar; end

  private

  def baz; end
end
```

```ruby
# EnforcedStyle: only_before

# bad
class Foo
  def bar; end
  private
  def baz; end
end

# good
class Foo
  def bar; end

  private
  def baz; end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `around` | `around`, `only_before` | Whether a blank line is required both before and after an access modifier, or only before it. |

## Blind spots

`in_macro_scope?`'s `class_constructor?` alternative (`Class.new`/
`Module.new`/`Struct.new`/`Data.define` blocks, as an alternative to being
nested in a bare `sclass`/`class`/`module`) is not modeled: a call-with-block
is always treated as transparent, regardless of what it is a call to. This is
strictly more permissive than upstream only for a block that is itself
nested somewhere other than directly under a class/module/root -- a shape
`class_constructor?` itself can only satisfy in the same narrow way (see the
module doc), so it does not diverge on any code this cop's own spec or the
corpus exercises. Separately, an explicit `begin...end` with a `rescue`/
`ensure` clause is `kwbegin`+`rescue`/`ensure` upstream (not the transparent
`kwbegin` alone), so an access modifier as the sole statement preceding a
`rescue`/`ensure` there is treated as in macro scope when upstream would not;
not exercised by the spec or the corpus. An access modifier used as an
`if`/`unless` condition (rather than its branch) is treated the same as one
in a branch (both transparent), since the ancestor chain alone cannot tell
which child slot was descended through; not meaningful Ruby, so it never
affects real code.
