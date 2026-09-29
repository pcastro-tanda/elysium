# Lint/Void

Checks for operators, literals, lambdas, and procs used in void context.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for operators, variables, literals, lambda, proc and nonmutating
methods used in void context.

`each` blocks are allowed to prevent false positives. For example, the
expression inside the `each` block below is not void, especially when the
receiver is an `Enumerator`:

```ruby
enumerator = [1, 2, 3].filter
enumerator.each { |item| item >= 2 } #=> [2, 3]
```

The last expression in an assignment method definition such as `def foo=(arg)`
is not flagged: Ruby discards it (the method returns its argument), but the
method can still be called directly and its return value relied upon, so
flagging it would be a false positive for this lint.

A constant used in a void context is flagged but not autocorrected, since
referencing a constant can trigger autoloading side effects (e.g. forcing a
file to load before a monkey-patch), so removing it may change behavior.

```ruby
# bad
def some_method
  some_num * 10
  do_something
end

# good
def some_method
  do_something
  some_num * 10
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| CheckForMethodsWithNoSideEffects | false |  | Whether to check for non-mutating methods that could be replaced by their mutating counterpart, or a call to `#each`. |

## Blind spots

A whitequark kwbegin/parenthesised-grouping body used as the *entire,
single-statement* body of a `def`/block (e.g. `def initialize; begin; 1; 2;
end; end`) is treated as a non-void context instead of transparently
inheriting the `def`'s/block's own void-ness, since the walk only knows a
node's kind once reached generically, not through its own container. No
fixture exercises this exotic style.
