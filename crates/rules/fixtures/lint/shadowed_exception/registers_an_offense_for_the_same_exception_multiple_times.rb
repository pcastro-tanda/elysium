begin
  something
rescue NameError, NameError
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  foo
end
