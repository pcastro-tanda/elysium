begin
  foo
rescue 'string', StandardError
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Rescuing from `'string'` will raise a `TypeError` instead of catching the actual exception.
  bar
end
