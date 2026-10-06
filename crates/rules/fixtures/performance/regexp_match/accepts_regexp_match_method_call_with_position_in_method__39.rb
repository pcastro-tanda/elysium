def foo
  return ::Regexp.last_match unless /re/.match(foo, 1)
end
