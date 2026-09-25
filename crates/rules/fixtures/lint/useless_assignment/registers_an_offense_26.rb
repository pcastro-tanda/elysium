begin
  do_something
  foo = true
  ^^^ Useless assignment to variable - `foo`.
rescue
  do_anything
end
