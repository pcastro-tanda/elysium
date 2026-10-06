def foo
  return if /re/.match(foo, 1)

  do_something($&)
end
