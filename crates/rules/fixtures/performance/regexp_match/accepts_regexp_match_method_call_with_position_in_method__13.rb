def foo
  if /re/.match(foo, 1)
    do_something($~)
  end
end
