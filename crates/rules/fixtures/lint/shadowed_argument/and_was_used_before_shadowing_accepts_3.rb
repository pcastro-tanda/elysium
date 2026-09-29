do_something do |foo|
  if bar
    puts foo
    foo = 43
  end
  foo = 42
  puts foo
end
