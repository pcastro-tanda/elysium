ERRORS = [Exception]
begin
  a = 3 / 0
rescue *ERRORS
  puts e
end
