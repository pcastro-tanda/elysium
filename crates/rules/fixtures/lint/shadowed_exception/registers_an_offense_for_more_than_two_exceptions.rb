begin
  something
rescue StandardError, NameError, NoMethodError
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  foo
end
