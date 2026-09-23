if foo
  do_something if obj&.ok? bar
               ^^ Consider merging nested conditions into outer `if` conditions.
end
