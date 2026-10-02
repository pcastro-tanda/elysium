def foo
  do_something
rescue ArgumentError
  # noop
rescue
^^^^^^ Useless `rescue` detected.
  raise
end
