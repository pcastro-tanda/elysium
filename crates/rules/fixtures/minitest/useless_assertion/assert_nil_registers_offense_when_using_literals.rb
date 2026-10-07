assert_nil []
^^^^^^^^^^^^^ Useless assertion detected.
assert_nil [foo]
^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_nil({})
^^^^^^^^^^^^^^ Useless assertion detected.
assert_nil({ key: value })
^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_nil nil
^^^^^^^^^^^^^^ Useless assertion detected.
assert_nil true
^^^^^^^^^^^^^^^ Useless assertion detected.
assert_nil false
^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_nil ""
^^^^^^^^^^^^^ Useless assertion detected.
assert_nil "foo"
^^^^^^^^^^^^^^^^ Useless assertion detected.

assert_nil 1
^^^^^^^^^^^^ Useless assertion detected.
