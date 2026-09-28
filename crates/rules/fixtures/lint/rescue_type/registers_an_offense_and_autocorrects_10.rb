begin
  foo
rescue StandardError, 0.0
^^^^^^^^^^^^^^^^^^^^^^^^^ Rescuing from `0.0` will raise a `TypeError` instead of catching the actual exception.
  bar
end
