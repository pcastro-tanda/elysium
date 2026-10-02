array.select do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `select` with a regexp match.
  x.match? /regexp/
end
