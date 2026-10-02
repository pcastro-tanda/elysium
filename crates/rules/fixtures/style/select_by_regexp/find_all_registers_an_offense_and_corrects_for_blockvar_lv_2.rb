lvar = /regexp/
array.find_all { |x| x !~ lvar }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a regexp match.
