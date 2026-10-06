def foo
  return if re =~ FOO

  do_something($1)
end
