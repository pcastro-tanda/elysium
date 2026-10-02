def foo
  do_something
rescue ArgumentError
  raise
rescue
  # noop
end
