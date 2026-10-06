def foo
  if FOO =~ re
    do_something(Regexp.last_match(1))
  end
end
