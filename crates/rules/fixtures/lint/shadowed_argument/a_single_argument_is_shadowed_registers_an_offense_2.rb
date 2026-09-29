do_something do |foo|
  foo = 42
  ^^^^^^^^ Argument `foo` was shadowed by a local variable before it was used.
  puts foo
end
