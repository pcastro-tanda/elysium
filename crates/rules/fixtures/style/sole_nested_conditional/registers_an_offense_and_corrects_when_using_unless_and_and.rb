unless (foo || bar)
  do_something if baz
               ^^ Consider merging nested conditions into outer `unless` conditions.
end
