begin
  a
rescue Exception
^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  b
rescue UnknownException
  c
end
