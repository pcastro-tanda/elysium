begin
  something
rescue NoMethodError, ZeroDivisionError
  handle_exception
else
  handle_non_exception
end
