def do_something(foo)
  lambda do
    puts foo
    foo = 43
  end

  foo = 42
  puts foo
end
