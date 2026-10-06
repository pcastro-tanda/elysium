x.map { |a| a[obj.do_something] }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `pluck(obj.do_something)` over `map { |a| a[obj.do_something] }`.
