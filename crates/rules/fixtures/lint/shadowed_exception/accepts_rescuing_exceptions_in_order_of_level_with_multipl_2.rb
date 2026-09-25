begin
  something
rescue NonStandardError, NoMethodError
  handle_standard_error
rescue Exception
  handle_exception
end
