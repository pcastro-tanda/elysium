do_something do |foo|
  lambda do
    puts foo
    foo = 43
  end

  foo = 42
  puts foo
end
