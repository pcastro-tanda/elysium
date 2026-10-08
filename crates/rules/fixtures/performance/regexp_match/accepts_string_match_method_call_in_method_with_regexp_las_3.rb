def foo
  return Regexp.last_match unless "foo".match(re)
end
