begin
  something
rescue Exception
^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  handle_exception
rescue NoMethodError, ZeroDivisionError
  handle_standard_error
end
