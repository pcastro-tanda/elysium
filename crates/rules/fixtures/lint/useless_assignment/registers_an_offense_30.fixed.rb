begin
  do_something
rescue FirstError
rescue SecondError
  p error # => nil
end
