lvar = /regexp/
array.select { |x| lvar !~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a regexp match.
