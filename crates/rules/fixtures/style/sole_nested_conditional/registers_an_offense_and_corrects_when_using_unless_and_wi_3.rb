unless foo && bar && baz
  do_something unless qux
               ^^^^^^ Consider merging nested conditions into outer `unless` conditions.
end
