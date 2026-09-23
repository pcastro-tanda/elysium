if baz && foo = bar
  if quux
  ^^ Consider merging nested conditions into outer `if` conditions.
    do_something
  end
end
