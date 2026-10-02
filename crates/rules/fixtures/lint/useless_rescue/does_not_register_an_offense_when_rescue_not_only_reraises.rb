def foo
  do_something
rescue
  do_cleanup
  raise e
end
