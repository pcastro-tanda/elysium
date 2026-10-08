def foo
  if foo.match(/re/)
    do_something(Regexp.last_match)
  end
end
