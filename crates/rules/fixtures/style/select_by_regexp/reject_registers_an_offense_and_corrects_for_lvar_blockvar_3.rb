lvar = /regexp/
array.reject { |x| lvar =~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a regexp match.
