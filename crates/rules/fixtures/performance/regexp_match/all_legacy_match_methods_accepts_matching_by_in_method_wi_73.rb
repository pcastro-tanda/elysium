def foo
  if FOO !~ re
    do_something($&)
  end
end
