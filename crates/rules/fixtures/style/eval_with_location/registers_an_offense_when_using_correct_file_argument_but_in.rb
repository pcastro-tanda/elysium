module_eval <<-CODE, __FILE__, 'bar'
                               ^^^^^ Incorrect line number for `module_eval`; use `__LINE__ + 1` instead of `'bar'`.
  do_something
CODE
