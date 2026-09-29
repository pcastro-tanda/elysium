begin
  foo
rescue => e
^^^^^^ Avoid rescuing without specifying an error class.
  bar
end
