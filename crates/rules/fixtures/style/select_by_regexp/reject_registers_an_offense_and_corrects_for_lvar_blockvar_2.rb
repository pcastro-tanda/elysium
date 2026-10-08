lvar = /regexp/
array&.reject { |x| lvar !~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `reject` with a regexp match.
