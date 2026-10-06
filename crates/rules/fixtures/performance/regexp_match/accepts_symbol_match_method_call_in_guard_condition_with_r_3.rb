def foo
  return if :foo.match(re)

  do_something(Regexp.last_match(1))
end
