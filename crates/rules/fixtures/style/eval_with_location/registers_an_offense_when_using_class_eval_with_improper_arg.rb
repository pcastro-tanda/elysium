class_eval <<-CODE, 'foo', 'bar'
                           ^^^^^ Incorrect line number for `class_eval`; use `__LINE__ + 1` instead of `'bar'`.
                    ^^^^^ Incorrect file for `class_eval`; use `__FILE__` instead of `'foo'`.
  do_something
CODE
