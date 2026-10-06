def foo
  if :foo !~ re
    do_something(Regexp.last_match(1))
  end
end
