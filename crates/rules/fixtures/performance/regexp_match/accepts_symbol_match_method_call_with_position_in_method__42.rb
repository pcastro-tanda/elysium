def foo
  return Regexp.last_match(1) if :foo.match(re, 1)
end
