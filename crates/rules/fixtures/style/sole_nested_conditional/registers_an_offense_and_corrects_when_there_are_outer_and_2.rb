# Outer comment.
if foo
  # Comment.
  if bar # nested condition
  ^^ Consider merging nested conditions into outer `if` conditions.
    do_something
  end
end
