begin
  foo
rescue StandardError, []
^^^^^^^^^^^^^^^^^^^^^^^^ Rescuing from `[]` will raise a `TypeError` instead of catching the actual exception.
  bar
end
