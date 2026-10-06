def foo
  return Regexp.last_match(1) unless FOO !~ re
end
