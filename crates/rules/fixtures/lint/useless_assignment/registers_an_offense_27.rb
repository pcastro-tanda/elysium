foo = false

begin
  do_something
rescue
  foo = true
  ^^^ Useless assignment to variable - `foo`.
  foo = true
end

puts foo
