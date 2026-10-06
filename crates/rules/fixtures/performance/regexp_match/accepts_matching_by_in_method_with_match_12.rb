def foo
  if foo !~ /re/
    do_something($MATCH)
  end
end
