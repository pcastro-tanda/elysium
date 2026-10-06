# ThreadSafety/ClassInstanceVariable

Avoid class instance variables.

| | |
| --- | --- |
| Department | ThreadSafety |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Avoid class instance variables: instance variables set or read in class methods are shared between threads.

## Options

This rule has no options.

## Blind spots

Upstream's `ancestor.children.first` test for a `define_method`/`define_singleton_method` call is reproduced for the common parent shapes (call receivers, blocks, lambdas, conditions, `and`/`or`, statements, parentheses, `begin`, arrays, `return`), not every node type.
