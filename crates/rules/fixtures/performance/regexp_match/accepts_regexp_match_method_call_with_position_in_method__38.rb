def foo
  return ::Regexp.last_match if /re/.match(foo, 1)
end
