def some_method
  if condition?
    foo = 1
    puts foo
    if other_condition?
      bar.each do |foo|
                   ^^^ Shadowing outer local variable - `foo`.
      end
    end
  elsif other_condition?
    bar.each do |foo|
    end
  else
    bar.each do |foo|
    end
  end
end
