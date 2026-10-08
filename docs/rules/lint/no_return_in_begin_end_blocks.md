# Lint/NoReturnInBeginEndBlocks

Do not `return` inside `begin..end` blocks in assignment contexts.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

```ruby
# bad
some_variable = begin
                  return if some_condition_is_met

                  some_value
                else
                  do_something
                end

# good
some_variable = if some_condition_is_met
                  return if another_condition_is_met

                  some_value
                else
                  do_something
                end
```

## Options

This rule has no options.

## Blind spots

Nested `begin...end` blocks are not specially deduplicated the way upstream's unconditional `each_node(:kwbegin)`/`each_node(:return)` searches implicitly allow (a `return` inside a nested `begin...end` could, upstream, be reported once per enclosing `begin...end` it's nested in); this port reports each `return` once, against its nearest enclosing explicit `begin...end` only. No fixture exercises a nested `begin...end`.
