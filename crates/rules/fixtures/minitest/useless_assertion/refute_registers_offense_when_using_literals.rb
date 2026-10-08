refute []
^^^^^^^^^ Useless assertion detected.
refute [foo]
^^^^^^^^^^^^ Useless assertion detected.

refute({})
^^^^^^^^^^ Useless assertion detected.
refute({ key: value })
^^^^^^^^^^^^^^^^^^^^^^ Useless assertion detected.

refute nil
^^^^^^^^^^ Useless assertion detected.
refute true
^^^^^^^^^^^ Useless assertion detected.
refute false
^^^^^^^^^^^^ Useless assertion detected.

refute ""
^^^^^^^^^ Useless assertion detected.
refute "foo"
^^^^^^^^^^^^ Useless assertion detected.

refute 1
^^^^^^^^ Useless assertion detected.
