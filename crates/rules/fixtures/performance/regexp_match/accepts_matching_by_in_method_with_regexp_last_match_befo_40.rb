def foo
  return ::Regexp.last_match unless FOO =~ re
end
