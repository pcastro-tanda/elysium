eval <<-CODE, binding, __FILE__, __LINE__ + 2
                                 ^^^^^^^^^^^^ Incorrect line number for `eval`; use `__LINE__ + 1` instead of `__LINE__ + 2`.
  do_something
CODE
