begin
  something
rescue ::Exception
^^^^^^^^^^^^^^^^^^ Avoid rescuing the `Exception` class. Perhaps you meant to rescue `StandardError`?
  #do nothing
end
