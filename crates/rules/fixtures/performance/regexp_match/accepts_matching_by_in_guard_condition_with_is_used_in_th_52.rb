def foo
  return if "foo" !~ re

  do_something($~)
end
