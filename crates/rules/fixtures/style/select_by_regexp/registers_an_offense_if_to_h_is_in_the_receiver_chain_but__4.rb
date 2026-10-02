foo.to_h.bar.reject { |x| x.match? /regexp/ }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a regexp match.
