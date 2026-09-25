begin
  something
rescue ZeroDivisionError
  handle_exception
rescue NoMethodError, StandardError
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  handle_standard_error
end
