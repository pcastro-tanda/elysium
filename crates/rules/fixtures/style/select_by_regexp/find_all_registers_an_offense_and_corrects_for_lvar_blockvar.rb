lvar = /regexp/
array.find_all { |x| lvar =~ x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a regexp match.
