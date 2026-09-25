foo = false

begin
  do_something
rescue
  true
  foo = true
end

puts foo
