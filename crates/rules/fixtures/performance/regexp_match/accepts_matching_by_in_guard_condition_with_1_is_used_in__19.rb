def foo
  return if FOO !~ re

  do_something($1)
end
