begin
  do_something
rescue FirstError => error
                     ^^^^^ Useless assignment to variable - `error`.
rescue SecondError
  p error # => nil
end
