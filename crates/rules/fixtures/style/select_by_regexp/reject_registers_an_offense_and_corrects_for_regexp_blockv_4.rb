array.reject { |x| /regexp/ =~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a regexp match.
