x.collect { |a| a[obj.do_something] }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `pluck(obj.do_something)` over `collect { |a| a[obj.do_something] }`.
