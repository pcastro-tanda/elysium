def foo
  return if foo.match(/re/, 1)

  do_something($100)
end
