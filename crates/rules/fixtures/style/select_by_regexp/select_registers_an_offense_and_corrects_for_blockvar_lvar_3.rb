lvar = /regexp/
array&.select { |x| x !~ lvar }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a regexp match.
