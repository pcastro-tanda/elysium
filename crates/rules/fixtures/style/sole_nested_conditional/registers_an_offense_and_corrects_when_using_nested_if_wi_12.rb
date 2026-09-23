unless foo & bar
  if baz
  ^^ Consider merging nested conditions into outer `unless` conditions.
    do_something
  end
end
