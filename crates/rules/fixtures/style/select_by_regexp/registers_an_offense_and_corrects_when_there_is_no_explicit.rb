array.filter { |x| x =~ y }
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a regexp match.
array.filter { |x| x =~ REGEXP }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a regexp match.
array.filter { |x| x =~ foo.bar.baz(quux) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a regexp match.
