lvar = /regexp/
array.reject { |x| x !~ lvar }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `reject` with a regexp match.
