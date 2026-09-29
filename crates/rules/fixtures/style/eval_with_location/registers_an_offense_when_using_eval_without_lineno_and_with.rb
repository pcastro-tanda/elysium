eval(<<-CODE, binding, __FILE__)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Pass a binding, `__FILE__`, and `__LINE__` to `eval`.
  do_something
CODE
