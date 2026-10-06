def foo
  if re !~ FOO
    do_something($`)
  end
end
