('aaa'...'abc').filter { |x| x.match?(/ab/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a regexp match.
