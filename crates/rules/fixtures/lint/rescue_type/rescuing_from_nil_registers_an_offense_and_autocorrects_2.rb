begin
  foo
rescue nil
^^^^^^^^^^ Rescuing from `nil` will raise a `TypeError` instead of catching the actual exception.
  bar
ensure
  baz
end
