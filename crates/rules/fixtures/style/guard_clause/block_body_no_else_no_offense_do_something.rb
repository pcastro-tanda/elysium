foo do
  test
  if something
    do_something(foo)
  end
end

foo do
  test
  unless something
    do_something(foo)
  end
end
