C.class_eval <<-CODE, __FILE__, __LINE__
                                ^^^^^^^^ Incorrect line number for `class_eval`; use `__LINE__ + 1` instead of `__LINE__`.
  do_something
CODE
