do_something do |foo|
                 ^^^ Argument `foo` was shadowed by a local variable before it was used.
  lambda do
    if baz
      foo = 43
    end
  end
  foo = 42
  puts foo
end
