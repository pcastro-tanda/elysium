def do_something
  klass = RuntimeError
  raise klass, 'hi'
end
