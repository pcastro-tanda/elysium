if foo || bar
  do_something if baz || qux
               ^^ Consider merging nested conditions into outer `if` conditions.
end
