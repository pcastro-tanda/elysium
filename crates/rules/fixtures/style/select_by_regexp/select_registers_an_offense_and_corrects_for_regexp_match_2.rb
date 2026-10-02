array.select { |x| !/regexp/.match?(x) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `select` with a regexp match.
