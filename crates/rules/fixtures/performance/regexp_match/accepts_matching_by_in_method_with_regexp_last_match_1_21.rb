def foo
  if /re/i === foo
    do_something(Regexp.last_match(1))
  end
end
