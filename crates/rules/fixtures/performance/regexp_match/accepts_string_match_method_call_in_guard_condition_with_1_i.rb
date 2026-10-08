def foo
  return if "foo".match(re)

  do_something($1)
end
