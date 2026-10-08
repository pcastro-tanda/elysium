array.find_all { /regexp/.match?(it) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a regexp match.
