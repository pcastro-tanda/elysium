if foo.is_a? Foo
  if bar && baz
  ^^ Consider merging nested conditions into outer `if` conditions.
    do_something if quux
                 ^^ Consider merging nested conditions into outer `if` conditions.
  end
end
