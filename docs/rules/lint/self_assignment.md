# Lint/SelfAssignment

Checks for self-assignments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for self-assignments.

```ruby
# bad
foo = foo
foo, bar = foo, bar
Foo = Foo
hash['foo'] = hash['foo']
obj.attr = obj.attr

# good
foo = bar
foo, bar = bar, foo
Foo = Bar
hash['foo'] = hash['bar']
obj.attr = obj.attr2

# good (method calls possibly can return different results)
hash[foo] = hash[foo]
```

With `AllowRBSInlineAnnotation: true` (default: `false`):

```ruby
# good
foo = foo #: Integer
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowRBSInlineAnnotation | false |  | Whether to allow a trailing `#: Type` RBS inline annotation to exempt an otherwise-flagged self-assignment. |

## Blind spots

`node.receiver == value_node.receiver`/`node_arguments == value_node.arguments` (the key/attribute
assignment shapes) are approximated by exact source-text comparison rather than RuboCop's true
structural `Node#==`; a genuinely equal expression written with different incidental formatting is
treated as unequal. `AllowRBSInlineAnnotation` only recognizes the annotation on a single-line
statement (see the module doc); a multi-line self-assignment with a trailing annotation is not
exempted.
