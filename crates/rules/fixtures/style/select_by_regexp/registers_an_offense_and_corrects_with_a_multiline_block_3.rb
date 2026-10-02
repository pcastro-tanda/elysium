array.find_all do |x|
^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a regexp match.
  x.match? /regexp/
end
