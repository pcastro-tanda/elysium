array.filter { /regexp/ !~ _1 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `filter` with a regexp match.
