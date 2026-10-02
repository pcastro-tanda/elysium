begin
  do_something
rescue => error
          ^^^^^ Useless assignment to variable - `error`.
end
