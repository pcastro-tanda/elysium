begin
  something
rescue NonStandardError, Exception
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  handle_exception
end
