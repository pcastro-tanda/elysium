begin
  raise
  ^^^^^ Always use `fail` to signal exceptions.
rescue Exception
  #do nothing
end
