def foo
  if re !~ "foo"
    do_something($MATCH)
  end
end
