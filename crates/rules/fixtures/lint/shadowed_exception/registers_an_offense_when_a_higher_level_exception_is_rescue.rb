begin
  something
rescue NoMethodError
  handle_no_method_error
rescue Exception
^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  handle_exception
rescue StandardError
  handle_standard_error
end
