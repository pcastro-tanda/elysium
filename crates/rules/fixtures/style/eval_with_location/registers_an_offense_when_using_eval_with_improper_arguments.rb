eval <<-CODE, binding, 'foo', 'bar'
                              ^^^^^ Incorrect line number for `eval`; use `__LINE__ + 1` instead of `'bar'`.
                       ^^^^^ Incorrect file for `eval`; use `__FILE__` instead of `'foo'`.
  do_something
CODE
