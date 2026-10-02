array.reject do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a regexp match.
  x.match? /regexp/
end
