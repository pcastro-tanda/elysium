def do_something(foo)
  if bar
    puts foo
    if baz
      foo = 43
    end
  end
  foo = 42
  puts foo
end
