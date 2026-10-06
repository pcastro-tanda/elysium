refute_empty []
^^^^^^^^^^^^^^^ Useless assertion detected.
refute_empty [foo]
^^^^^^^^^^^^^^^^^^ Useless assertion detected.

refute_empty({})
^^^^^^^^^^^^^^^^ Useless assertion detected.
refute_empty({ key: value })
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless assertion detected.

refute_empty nil
^^^^^^^^^^^^^^^^ Useless assertion detected.
refute_empty true
^^^^^^^^^^^^^^^^^ Useless assertion detected.
refute_empty false
^^^^^^^^^^^^^^^^^^ Useless assertion detected.

refute_empty ""
^^^^^^^^^^^^^^^ Useless assertion detected.
refute_empty "foo"
^^^^^^^^^^^^^^^^^^ Useless assertion detected.

refute_empty 1
^^^^^^^^^^^^^^ Useless assertion detected.
