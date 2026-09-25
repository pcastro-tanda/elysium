foo = false

begin
  do_something
rescue
  true
  foo = true
ensure
  do_anything
end

puts foo
