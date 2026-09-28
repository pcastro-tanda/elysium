def foobar
  foo
rescue "#{string}"
^^^^^^^^^^^^^^^^^^ Rescuing from `"#{string}"` will raise a `TypeError` instead of catching the actual exception.
  bar
end
