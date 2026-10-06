def foo
  return Regexp.last_match(1) if foo =~ /re/
end
