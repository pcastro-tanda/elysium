begin
  do_something
  :in_begin
rescue FirstError
  :in_first_rescue
rescue SecondError
  :in_second_rescue
else
  :in_else
ensure
  foo = :in_ensure
end

puts foo
