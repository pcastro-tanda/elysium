def do_something(foo)
  foo = 42
  ^^^^^^^^ Argument `foo` was shadowed by a local variable before it was used.
  puts foo
end
