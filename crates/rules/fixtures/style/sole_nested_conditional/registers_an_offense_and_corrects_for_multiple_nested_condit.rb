if foo
  if bar
  ^^ Consider merging nested conditions into outer `if` conditions.
    if baz
    ^^ Consider merging nested conditions into outer `if` conditions.
      do_something
    end
  end
end
