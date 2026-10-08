def foo
  return if FOO !~ re

  do_something($MATCH)
end
