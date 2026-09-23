if foo & bar
  if baz
  ^^ Consider merging nested conditions into outer `if` conditions.
    do_something
  end
end
