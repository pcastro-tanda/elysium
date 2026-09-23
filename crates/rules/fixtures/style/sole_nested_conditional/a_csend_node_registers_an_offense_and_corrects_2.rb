if foo
  if bar&.baz quux
  ^^ Consider merging nested conditions into outer `if` conditions.
    do_something
  end
end
