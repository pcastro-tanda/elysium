def do_something(foo)
  lambda do
    puts foo

    something do
      foo = 43
    end
  end

  foo = 42
  puts foo
end
