def foo
  return Regexp.last_match if foo.match(/re/)
end
