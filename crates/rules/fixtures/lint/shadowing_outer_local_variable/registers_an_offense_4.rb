def some_method
  foo = 1
  puts foo
  1.times do |i; foo|
                 ^^^ Shadowing outer local variable - `foo`.
    puts foo
  end
end
