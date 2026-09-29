begin
  foo
rescue StandardError
^^^^^^^^^^^^^^^^^^^^ Omit the error class when rescuing `StandardError` by itself.
  bar
end
