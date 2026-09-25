# Lint/ShadowingOuterLocalVariable

Do not use the same name as outer local variable for block arguments or block local variables.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for the use of local variable names from an outer scope in block
arguments or block-local variables. This mirrors the warning given by
`ruby -cw` prior to Ruby 2.6: "shadowing outer local variable - foo".

The cop is now disabled by default to match the upstream Ruby behavior.
It's useful, however, if you'd like to avoid shadowing variables from outer
scopes, which some people consider an anti-pattern that makes it harder to
keep track of what's going on in a program.

NOTE: Shadowing of variables in block passed to `Ractor.new` is allowed
because `Ractor` should not access outer variables.

```ruby
# bad
def some_method
  foo = 1

  2.times do |foo| # shadowing outer `foo`
    do_something(foo)
  end
end

# good
def some_method
  foo = 1

  2.times do |bar|
    do_something(bar)
  end
end
```

## Options

This rule has no options.

## Blind spots

`same_conditions_node_different_branch?` compares the node that encloses
the shadowing block with the conditional that encloses the outer
declaration. Prism wraps clause bodies in a `StatementsNode` (and `else`
bodies in an `ElseNode`) where whitequark's parser emits the bare statement
or a `begin`; a single-statement `StatementsNode` is therefore treated as
transparent. A clause whose body is a parenthesised expression list
(`(a; b)`) keeps the extra `ParenthesesNode`, so the comparison can miss
there.
