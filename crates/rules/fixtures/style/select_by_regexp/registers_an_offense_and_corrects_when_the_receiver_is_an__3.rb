[].find_all { |x| x.match?(/regexp/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a regexp match.
foo.to_a.find_all { |x| x.match?(/regexp/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a regexp match.
