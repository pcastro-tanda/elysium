lvar = /regexp/
array&.filter { |x| x !~ lvar }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a regexp match.
