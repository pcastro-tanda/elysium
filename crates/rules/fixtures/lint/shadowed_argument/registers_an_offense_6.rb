do_something do |foo, bar|
  lambda do
    bar = 42
  end

  foo = 43
  ^^^^^^^^ Argument `foo` was shadowed by a local variable before it was used.
  puts(foo, bar)
end
