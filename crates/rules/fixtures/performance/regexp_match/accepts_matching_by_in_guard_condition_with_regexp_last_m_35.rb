def foo
  return if foo !~ /re/

  do_something(::Regexp.last_match)
end
