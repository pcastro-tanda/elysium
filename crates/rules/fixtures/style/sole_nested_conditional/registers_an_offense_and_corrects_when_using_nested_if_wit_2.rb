unless foo
  if bar
  ^^ Consider merging nested conditions into outer `unless` conditions.
    do_something
  end
end
