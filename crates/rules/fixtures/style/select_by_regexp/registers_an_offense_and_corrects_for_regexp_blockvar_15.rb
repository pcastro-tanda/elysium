array.find_all { /regexp/ !~ _1 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a regexp match.
