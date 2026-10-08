array.select { /regexp/ !~ it }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a regexp match.
