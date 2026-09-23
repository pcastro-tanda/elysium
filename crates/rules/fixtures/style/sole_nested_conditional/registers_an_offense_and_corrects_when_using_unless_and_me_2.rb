unless foo.is_a?(Foo)
  do_something if bar
               ^^ Consider merging nested conditions into outer `unless` conditions.
end
