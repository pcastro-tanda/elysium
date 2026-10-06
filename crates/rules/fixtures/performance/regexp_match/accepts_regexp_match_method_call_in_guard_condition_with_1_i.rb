def foo
  return if /re/.match(foo)

  do_something($1)
end
