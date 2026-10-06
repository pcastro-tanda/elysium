def foo
  return Regexp.last_match unless foo.match(/re/, 1)
end
