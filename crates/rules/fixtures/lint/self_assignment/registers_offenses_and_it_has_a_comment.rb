foo = foo #: Integer
^^^^^^^^^ Self-assignment detected.
@foo = @foo #: Integer
^^^^^^^^^^^ Self-assignment detected.
@@foo = @@foo #: Integer
^^^^^^^^^^^^^ Self-assignment detected.
$foo = $foo #: Integer
^^^^^^^^^^^ Self-assignment detected.
Foo = Foo #: Integer
^^^^^^^^^ Self-assignment detected.
foo, bar = foo, bar #: Integer
^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo ||= foo #: Integer
^^^^^^^^^^^ Self-assignment detected.
foo &&= foo #: Integer
^^^^^^^^^^^ Self-assignment detected.
foo.bar = foo.bar #: Integer
^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo&.bar = foo&.bar #: Integer
^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo["bar"] = foo["bar"] #: Integer
^^^^^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo&.[]=("bar", foo["bar"]) #: Integer
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
