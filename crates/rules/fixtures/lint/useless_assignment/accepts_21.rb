begin
  do_something
  foo = :in_begin
rescue FirstError
  foo = :in_first_rescue
rescue SecondError
  foo = :in_second_rescue
else
  foo = :in_else
end

puts foo
