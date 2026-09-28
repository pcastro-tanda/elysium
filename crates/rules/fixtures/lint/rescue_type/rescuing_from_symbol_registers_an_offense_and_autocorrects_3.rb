def foobar
  foo
rescue :symbol
^^^^^^^^^^^^^^ Rescuing from `:symbol` will raise a `TypeError` instead of catching the actual exception.
  bar
end
