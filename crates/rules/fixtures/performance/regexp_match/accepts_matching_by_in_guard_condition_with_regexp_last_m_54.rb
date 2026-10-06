def foo
  return if re !~ FOO

  do_something(Regexp.last_match(1))
end
