def do_something(foo)
                 ^^^ Argument `foo` was shadowed by a local variable before it was used.
  something { foo = 43 }

  foo = 42
  puts foo
end
