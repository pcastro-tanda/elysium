def foo
  return if "foo".match(re)

  do_something($2)
end
