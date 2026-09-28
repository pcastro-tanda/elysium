def foobar
  foo
rescue {}
^^^^^^^^^ Rescuing from `{}` will raise a `TypeError` instead of catching the actual exception.
  bar
ensure
  baz
end
