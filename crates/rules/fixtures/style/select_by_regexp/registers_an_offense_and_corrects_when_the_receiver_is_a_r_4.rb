('aaa'...'abc').reject { |x| x.match?(/ab/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a regexp match.
