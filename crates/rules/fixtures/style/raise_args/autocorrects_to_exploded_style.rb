def do_something
  klass = RuntimeError
  raise klass.new('hi')
  ^^^^^^^^^^^^^^^^^^^^^ Provide an exception class and message as arguments to `raise`.
end
