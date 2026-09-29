def do_something(foo)
                 ^^^ Argument `foo` was shadowed by a local variable before it was used.
  if bar
    foo = 43
  end
  foo = 42
  puts foo
end
