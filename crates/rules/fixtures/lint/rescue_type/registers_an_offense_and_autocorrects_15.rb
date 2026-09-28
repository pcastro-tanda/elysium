begin
  foo
rescue :symbol, StandardError
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Rescuing from `:symbol` will raise a `TypeError` instead of catching the actual exception.
  bar
end
