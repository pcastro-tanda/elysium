def foo
  return if "foo" !~ re

  do_something($1)
end
