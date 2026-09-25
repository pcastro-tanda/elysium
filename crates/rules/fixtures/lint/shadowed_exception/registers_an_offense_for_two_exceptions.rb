begin
  something
rescue StandardError, NameError
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  foo
end
