def foo
  return if "foo".match(re, 1)

  do_something($&)
end
