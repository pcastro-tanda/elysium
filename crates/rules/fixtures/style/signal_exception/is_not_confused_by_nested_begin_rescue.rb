begin
  raise
  ^^^^^ Use `fail` instead of `raise` to signal exceptions.
  begin
    raise
    ^^^^^ Use `fail` instead of `raise` to signal exceptions.
  rescue
    fail
    ^^^^ Use `raise` instead of `fail` to rethrow exceptions.
  end
rescue Exception
  #do nothing
end
