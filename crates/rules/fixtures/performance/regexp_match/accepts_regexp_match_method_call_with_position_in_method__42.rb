def foo
  return Regexp.last_match(1) if /re/.match(foo, 1)
end
