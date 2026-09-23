if foo || bar
  if baz || qux
  ^^ Consider merging nested conditions into outer `if` conditions.
    do_something
  end
end
