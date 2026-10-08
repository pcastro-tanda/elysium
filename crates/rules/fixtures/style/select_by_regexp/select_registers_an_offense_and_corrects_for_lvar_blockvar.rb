lvar = /regexp/
array.select { |x| lvar =~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `select` with a regexp match.
