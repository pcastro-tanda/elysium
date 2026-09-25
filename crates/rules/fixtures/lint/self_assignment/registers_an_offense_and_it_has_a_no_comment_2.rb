foo = foo
^^^^^^^^^ Self-assignment detected.
@foo = @foo
^^^^^^^^^^^ Self-assignment detected.
@@foo = @@foo
^^^^^^^^^^^^^ Self-assignment detected.
$foo = $foo
^^^^^^^^^^^ Self-assignment detected.
Foo = Foo
^^^^^^^^^ Self-assignment detected.
foo, bar = foo, bar
^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo ||= foo
^^^^^^^^^^^ Self-assignment detected.
foo &&= foo
^^^^^^^^^^^ Self-assignment detected.
foo.bar = foo.bar
^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo&.bar = foo&.bar
^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo["bar"] = foo["bar"]
^^^^^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo&.[]=("bar", foo["bar"])
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Self-assignment detected.

foo = foo # comment
^^^^^^^^^ Self-assignment detected.
@foo = @foo # comment
^^^^^^^^^^^ Self-assignment detected.
@@foo = @@foo # comment
^^^^^^^^^^^^^ Self-assignment detected.
$foo = $foo # comment
^^^^^^^^^^^ Self-assignment detected.
Foo = Foo # comment
^^^^^^^^^ Self-assignment detected.
foo, bar = foo, bar # comment
^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo ||= foo # comment
^^^^^^^^^^^ Self-assignment detected.
foo &&= foo # comment
^^^^^^^^^^^ Self-assignment detected.
foo.bar = foo.bar # comment
^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo&.bar = foo&.bar # comment
^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo["bar"] = foo["bar"] # comment
^^^^^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
foo&.[]=("bar", foo["bar"]) # comment
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Self-assignment detected.
