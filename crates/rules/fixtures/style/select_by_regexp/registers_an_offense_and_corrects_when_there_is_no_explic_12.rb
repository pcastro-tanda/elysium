array&.find_all { |x| x !~ y }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a regexp match.
array&.find_all { |x| x !~ REGEXP }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a regexp match.
array&.find_all { |x| x !~ foo.bar.baz(quux) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `find_all` with a regexp match.
