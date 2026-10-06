def foo
  return Regexp.last_match(1) unless /re/.match(foo, 1)
end
