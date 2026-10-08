def foo
  return Regexp.last_match(1) unless /re/i === foo
end
