do_something(**{foo: bar, baz: qux}.merge(options1, options2).merge(options3))
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Remove the redundant double splat and braces, use keyword arguments directly.
