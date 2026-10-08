def foo
  do_something
rescue => e
  raise
ensure
  do_something(e)
end
