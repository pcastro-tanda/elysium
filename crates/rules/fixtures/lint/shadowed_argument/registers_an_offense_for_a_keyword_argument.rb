def do_something(foo:)
  foo = 2
  ^^^^^^^ Argument `foo` was shadowed by a local variable before it was used.
  puts foo
end
