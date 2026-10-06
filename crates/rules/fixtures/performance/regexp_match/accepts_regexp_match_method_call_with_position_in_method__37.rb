def foo
  if /re/.match(foo, 1)
    do_something(::Regexp.last_match)
  end
end
