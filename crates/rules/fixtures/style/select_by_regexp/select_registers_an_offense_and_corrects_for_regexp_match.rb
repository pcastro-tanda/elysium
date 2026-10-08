array.select { |x| /regexp/.match? x }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `select` with a regexp match.
