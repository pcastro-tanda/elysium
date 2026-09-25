def some_method
  foo = 1
  ^^^ Useless assignment to variable - `foo`.
  1.times do |foo|
    puts foo
  end
end
