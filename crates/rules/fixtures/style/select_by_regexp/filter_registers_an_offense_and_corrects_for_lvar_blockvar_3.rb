lvar = /regexp/
array&.filter { |x| lvar !~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a regexp match.
