begin
  something
rescue NonStandardError, OtherError
  handle_standard_error
rescue CustomError
  handle_exception
end
