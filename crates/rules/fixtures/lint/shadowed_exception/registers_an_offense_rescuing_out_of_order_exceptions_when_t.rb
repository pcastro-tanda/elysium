begin
  something
rescue Exception
^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  handle_exception
rescue StandardError
  handle_standard_error
ensure
  everything_is_ok
end
