def foo
  return ::Regexp.last_match if /re/i === foo
end
