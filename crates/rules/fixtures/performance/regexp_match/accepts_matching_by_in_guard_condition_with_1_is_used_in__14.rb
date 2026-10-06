def foo
  return if re !~ "foo"

  do_something($1)
end
