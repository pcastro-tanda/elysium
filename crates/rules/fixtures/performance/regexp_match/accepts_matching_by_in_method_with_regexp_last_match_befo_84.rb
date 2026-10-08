def foo
  return ::Regexp.last_match unless /re/i === foo
end
