def do_something(foo)
  if baz
    puts foo
    lambda do
      foo = 43
    end
  end
  foo = 42
  puts foo
end
