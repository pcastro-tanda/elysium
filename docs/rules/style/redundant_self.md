# Style/RedundantSelf

Checks for redundant uses of `self`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The usage of `self` is only needed when:

* Sending a message to same object with zero arguments in
  presence of a method name clash with an argument or a local
  variable.

* Calling an attribute writer to prevent a local variable assignment.

Note, with using explicit self you can only send messages with public or
protected scope, you cannot send private messages this way.

Note we allow uses of `self` with operators because it would be awkward
otherwise. Also allows the use of `self.it` without arguments in blocks,
as in `0.times { self.it }`, following `Lint/ItWithoutArgumentsInBlock` cop.

```ruby
# bad
def foo(bar)
  self.baz
end

# good
def foo(bar)
  self.bar  # Resolves name clash with the argument.
end

def foo
  bar = 1
  self.bar  # Resolves name clash with the local variable.
end

def foo
  %w[x y z].select do |bar|
    self.bar == bar  # Resolves name clash with argument of the block.
  end
end
```

## Options

This rule has no options.

## Blind spots

`@allowed_send_nodes` (`allow_self`) and `node.parent&.mlhs_type?` are
omitted: both exist upstream only because whitequark represents
`self.foo += 1`/`self.foo ||= x` and a multi-assignment attribute target
(`self.a, foo.b = x`) as an ordinary `send` node nested inside an
`op_asgn`/`or_asgn`/`masgn` wrapper, which the generic traversal also
visits as a plain send. Prism gives every one of those shapes its own
dedicated node kind instead (`CallOrWriteNode`, `CallAndWriteNode`,
`CallOperatorWriteNode`, `CallTargetNode`, ...), none of which is a
`CallNode`, so they never reach this rule's send handling and need no
special-casing.

`KERNEL_METHODS` is a build-time dump of `Kernel.methods(false)` on Ruby
3.4.2 (see the constant's doc comment), matching RuboCop's own
load-time evaluation on whatever Ruby runs RuboCop.
