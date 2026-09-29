def some_method(foo, bar, &block)
                           ^^^^^ Unused method argument - `block`. If it's necessary, use `_` or `_block` as an argument name to indicate that it won't be used. If it's unnecessary, remove it.
  foo + bar
end
