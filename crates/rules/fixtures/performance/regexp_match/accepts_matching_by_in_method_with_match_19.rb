def foo
  if FOO !~ re
    do_something($MATCH)
  end
end
