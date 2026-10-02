array.filter { |x| x !~ /regexp/ }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a regexp match.
