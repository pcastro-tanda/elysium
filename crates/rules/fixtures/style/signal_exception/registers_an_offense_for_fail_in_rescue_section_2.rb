begin
  raise
rescue Exception
  fail
  ^^^^ Always use `raise` to signal exceptions.
end
