def foo
  return Regexp.last_match if "foo" !~ re
end
