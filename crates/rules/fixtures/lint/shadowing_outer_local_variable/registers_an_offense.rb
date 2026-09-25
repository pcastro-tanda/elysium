def some_method
  foo = 1
  puts foo
  1.times do |foo|
              ^^^ Shadowing outer local variable - `foo`.
  end
end
