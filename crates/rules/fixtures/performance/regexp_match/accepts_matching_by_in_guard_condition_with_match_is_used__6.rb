def foo
  return if :foo =~ re

  do_something($MATCH)
end
