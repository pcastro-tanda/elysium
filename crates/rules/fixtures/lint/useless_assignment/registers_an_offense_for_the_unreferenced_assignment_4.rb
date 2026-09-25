def some_method
  foo = 1
  ^^^ Useless assignment to variable - `foo`.
  (foo = do_something_returns_object_or_nil) && do_something
  foo
end
