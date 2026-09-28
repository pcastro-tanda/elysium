begin
  foo
rescue 0.0, StandardError
^^^^^^^^^^^^^^^^^^^^^^^^^ Rescuing from `0.0` will raise a `TypeError` instead of catching the actual exception.
  bar
end
