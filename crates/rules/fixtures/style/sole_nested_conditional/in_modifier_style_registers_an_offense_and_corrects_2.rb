if foo
  if ok?(bar)
  ^^ Consider merging nested conditions into outer `if` conditions.
    do_something
  end
end
