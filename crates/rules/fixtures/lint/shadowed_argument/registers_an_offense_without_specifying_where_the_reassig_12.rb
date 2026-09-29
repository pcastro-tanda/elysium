do_something do |foo|
                 ^^^ Argument `foo` was shadowed by a local variable before it was used.
  if baz
    lambda do
      foo = 43
    end
  end

  foo = 42
  puts foo
end
