lvar = /regexp/
array.reject { |x| x =~ lvar }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a regexp match.
