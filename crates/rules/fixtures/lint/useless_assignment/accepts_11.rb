def some_method
  foo = 1
  do_something_returns_object_or_nil && foo = 2
  foo
end
