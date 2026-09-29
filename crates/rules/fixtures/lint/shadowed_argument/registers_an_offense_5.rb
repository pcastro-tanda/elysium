do_something do |foo|
  foo = 43
  ^^^^^^^^ Argument `foo` was shadowed by a local variable before it was used.
  something { foo = 42 }
  puts foo
end
