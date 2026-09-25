def some_method
  foo = 1

  if cond
    bar.each do |foo|
                 ^^^ Shadowing outer local variable - `foo`.
    end
  end
end
