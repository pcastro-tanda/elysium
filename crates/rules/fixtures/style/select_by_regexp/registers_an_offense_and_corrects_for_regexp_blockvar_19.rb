array.find_all { |x| /regexp/ !~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a regexp match.
