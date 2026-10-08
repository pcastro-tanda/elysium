array.reject { |x| x !~ y }
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `reject` with a regexp match.
array.reject { |x| x !~ REGEXP }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `reject` with a regexp match.
array.reject { |x| x !~ foo.bar.baz(quux) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `reject` with a regexp match.
