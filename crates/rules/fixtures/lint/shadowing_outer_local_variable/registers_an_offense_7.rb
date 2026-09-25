if condition?
  foo = 1
  puts foo
  bar.each do |foo|
               ^^^ Shadowing outer local variable - `foo`.
  end
else
  bar.each do |foo|
  end
end
