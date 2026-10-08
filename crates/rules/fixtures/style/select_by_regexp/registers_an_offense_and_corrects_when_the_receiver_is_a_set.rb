Set.new.filter { |x| x.match?(/regexp/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a regexp match.
[].to_set.filter { |x| x.match?(/regexp/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a regexp match.
