def do_something(foo, &)
  if foo
    bar(foo)
  else
    bar(&)
  end
end
