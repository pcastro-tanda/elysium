def foo
  return Regexp.last_match(1) unless foo.match(/re/, 1)
end
