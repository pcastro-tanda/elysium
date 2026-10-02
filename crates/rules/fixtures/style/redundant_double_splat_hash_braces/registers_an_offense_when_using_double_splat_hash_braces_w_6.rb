do_something(**{ foo: bar }.merge(options))
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Remove the redundant double splat and braces, use keyword arguments directly.
do_something(**{ baz: qux }.merge(options))
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Remove the redundant double splat and braces, use keyword arguments directly.
