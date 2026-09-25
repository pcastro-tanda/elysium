begin
  do_something
rescue
  do_anything
ensure
  foo = true
  ^^^ Useless assignment to variable - `foo`.
  foo = true
end

puts foo
