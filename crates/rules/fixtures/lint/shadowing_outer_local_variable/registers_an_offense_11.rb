def some_method
  case condition
  when foo then
    foo = 1
    puts foo
    bar.each do |foo|
                 ^^^ Shadowing outer local variable - `foo`.
    end
  when bar then
    bar.each do |foo|
    end
  else
    bar.each do |foo|
    end
  end
end
