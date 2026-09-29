do_something do |foo|
  lambda do
    puts foo
    if baz
      foo = 43
    end
  end
  foo = 42
  puts foo
end
