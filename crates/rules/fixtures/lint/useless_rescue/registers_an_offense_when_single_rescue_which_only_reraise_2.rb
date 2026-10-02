def foo
  do_something
rescue
^^^^^^ Useless `rescue` detected.
  raise $!
end
