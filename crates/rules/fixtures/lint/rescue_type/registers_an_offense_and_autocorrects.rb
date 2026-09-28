begin
  foo
rescue nil, StandardError
^^^^^^^^^^^^^^^^^^^^^^^^^ Rescuing from `nil` will raise a `TypeError` instead of catching the actual exception.
  bar
end
