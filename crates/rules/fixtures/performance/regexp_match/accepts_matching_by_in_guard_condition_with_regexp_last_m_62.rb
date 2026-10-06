def foo
  return if /re/i === foo

  do_something(::Regexp.last_match)
end
