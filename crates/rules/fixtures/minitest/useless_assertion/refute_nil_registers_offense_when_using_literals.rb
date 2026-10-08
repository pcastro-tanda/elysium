refute_nil []
^^^^^^^^^^^^^ Useless assertion detected.
refute_nil [foo]
^^^^^^^^^^^^^^^^ Useless assertion detected.

refute_nil({})
^^^^^^^^^^^^^^ Useless assertion detected.
refute_nil({ key: value })
^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless assertion detected.

refute_nil nil
^^^^^^^^^^^^^^ Useless assertion detected.
refute_nil true
^^^^^^^^^^^^^^^ Useless assertion detected.
refute_nil false
^^^^^^^^^^^^^^^^ Useless assertion detected.

refute_nil ""
^^^^^^^^^^^^^ Useless assertion detected.
refute_nil "foo"
^^^^^^^^^^^^^^^^ Useless assertion detected.

refute_nil 1
^^^^^^^^^^^^ Useless assertion detected.
