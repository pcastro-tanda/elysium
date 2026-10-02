array.filter do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a regexp match.
  x.match? /regexp/
end
