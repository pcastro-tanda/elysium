begin
  something
rescue NoMethodError, ZeroDivisionError
  handle_standard_error
rescue Exception
  handle_exception
end
