def foo
  do_something
rescue => e
^^^^^^^^^^^ Useless `rescue` detected.
  raise e
end
