def foo
  do_something
rescue
  Thread.current.raise
end
