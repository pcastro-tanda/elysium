def foo
  return if re =~ FOO

  do_something($MATCH)
end
