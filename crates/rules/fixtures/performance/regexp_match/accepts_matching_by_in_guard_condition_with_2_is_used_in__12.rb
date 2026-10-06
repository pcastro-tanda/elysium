def foo
  return if foo !~ /re/

  do_something($2)
end
