do_something do |foo|
  foo = 43
  ^^^^^^^^ Argument `foo` was shadowed by a local variable before it was used.
  if bar
    foo = 42
  end
  puts foo
end
