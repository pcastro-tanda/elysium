def foo
  if /re/.match(foo)
    do_something(Regexp.last_match(1))
  end
end
