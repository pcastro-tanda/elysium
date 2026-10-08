array.find_all { /regexp/ !~ it }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a regexp match.
