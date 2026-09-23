unless foo.bar arg1, arg2
  do_something if baz
               ^^ Consider merging nested conditions into outer `unless` conditions.
end
