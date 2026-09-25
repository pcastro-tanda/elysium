def some_method
  (foo = do_something_returns_object_or_nil) && (foo = 1)
  foo
end
