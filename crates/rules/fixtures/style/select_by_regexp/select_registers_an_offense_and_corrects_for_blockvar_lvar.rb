lvar = /regexp/
array.select { |x| x =~ lvar }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `select` with a regexp match.
