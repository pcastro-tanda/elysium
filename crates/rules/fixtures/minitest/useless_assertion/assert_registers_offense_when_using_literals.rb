assert []
^^^^^^^^^ Useless assertion detected.
assert [foo]
^^^^^^^^^^^^ Useless assertion detected.

assert({})
^^^^^^^^^^ Useless assertion detected.
assert({ key: value })
^^^^^^^^^^^^^^^^^^^^^^ Useless assertion detected.

assert nil
^^^^^^^^^^ Useless assertion detected.
assert true
^^^^^^^^^^^ Useless assertion detected.
assert false
^^^^^^^^^^^^ Useless assertion detected.

assert ""
^^^^^^^^^ Useless assertion detected.
assert "foo"
^^^^^^^^^^^^ Useless assertion detected.

assert 1
^^^^^^^^ Useless assertion detected.
