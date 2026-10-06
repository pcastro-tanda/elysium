assert_not []
^^^^^^^^^^^^^ Useless assertion detected.
assert_not [foo]
^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_not({})
^^^^^^^^^^^^^^ Useless assertion detected.
assert_not({ key: value })
^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_not nil
^^^^^^^^^^^^^^ Useless assertion detected.
assert_not true
^^^^^^^^^^^^^^^ Useless assertion detected.
assert_not false
^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_not ""
^^^^^^^^^^^^^ Useless assertion detected.
assert_not "foo"
^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_not 1
^^^^^^^^^^^^ Useless assertion detected.
