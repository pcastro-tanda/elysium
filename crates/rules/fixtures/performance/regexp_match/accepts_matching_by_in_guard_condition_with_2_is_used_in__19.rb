def foo
  return if FOO !~ re

  do_something($2)
end
