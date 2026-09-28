begin
  foo
rescue StandardError, nil
^^^^^^^^^^^^^^^^^^^^^^^^^ Rescuing from `nil` will raise a `TypeError` instead of catching the actual exception.
  bar
end
