def foo
  return ::Regexp.last_match if foo.match(/re/, 1)
end
