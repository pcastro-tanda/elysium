while foo
  var = do_something
end

if bar
  array.each do |var|
                 ^^^ Shadowing outer local variable - `var`.
  end
end
