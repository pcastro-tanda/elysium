def foobar
  foo
rescue StandardError => e
^^^^^^^^^^^^^^^^^^^^ Omit the error class when rescuing `StandardError` by itself.
  bar
end
