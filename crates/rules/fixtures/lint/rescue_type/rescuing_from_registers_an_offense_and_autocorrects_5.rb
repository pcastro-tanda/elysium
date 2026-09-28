begin
  foo
rescue {}
^^^^^^^^^ Rescuing from `{}` will raise a `TypeError` instead of catching the actual exception.
  bar
end
