array&.filter { |x| /regexp/ !~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a regexp match.
