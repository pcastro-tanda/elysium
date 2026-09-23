if foo
  do_something unless bar & baz
               ^^^^^^ Consider merging nested conditions into outer `if` conditions.
end
