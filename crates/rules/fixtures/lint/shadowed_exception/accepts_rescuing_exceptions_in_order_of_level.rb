begin
  something
rescue StandardError
  handle_standard_error
rescue Exception
  handle_exception
end
