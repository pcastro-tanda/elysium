begin
  fail
rescue Exception
  raise
  ^^^^^ Always use `fail` to signal exceptions.
end
