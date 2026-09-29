foo.instance_eval <<-CODE
^^^^^^^^^^^^^^^^^^^^^^^^^ Pass `__FILE__` and `__LINE__` to `instance_eval`.
  do_something
CODE
