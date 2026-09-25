begin
  do_something
  foo = :in_begin
  ^^^ Useless assignment to variable - `foo`.
rescue FirstError
  foo = :in_first_rescue
  ^^^ Useless assignment to variable - `foo`.
rescue SecondError
  foo = :in_second_rescue
  ^^^ Useless assignment to variable - `foo`.
else
  foo = :in_else
  ^^^ Useless assignment to variable - `foo`.
ensure
  foo = :in_ensure
end

puts foo
