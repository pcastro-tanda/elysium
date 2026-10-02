Set.new.find_all { |x| x.match?(/regexp/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a regexp match.
[].to_set.find_all { |x| x.match?(/regexp/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a regexp match.
