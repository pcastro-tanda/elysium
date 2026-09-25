def foo
  something
rescue StandardError, RuntimeError
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not shadow rescued Exceptions.
  handle_exception
end
