ERRORS = [FirstError, SecondError]
begin
  foo
rescue *ERRORS
  bar
end
