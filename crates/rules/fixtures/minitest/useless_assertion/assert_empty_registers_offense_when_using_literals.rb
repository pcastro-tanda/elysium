assert_empty []
^^^^^^^^^^^^^^^ Useless assertion detected.
assert_empty [foo]
^^^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_empty({})
^^^^^^^^^^^^^^^^ Useless assertion detected.
assert_empty({ key: value })
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_empty nil
^^^^^^^^^^^^^^^^ Useless assertion detected.
assert_empty true
^^^^^^^^^^^^^^^^^ Useless assertion detected.
assert_empty false
^^^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_empty ""
^^^^^^^^^^^^^^^ Useless assertion detected.
assert_empty "foo"
^^^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_empty 1
^^^^^^^^^^^^^^ Useless assertion detected.
