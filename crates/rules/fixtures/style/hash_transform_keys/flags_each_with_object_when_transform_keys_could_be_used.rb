{a: 1, b: 2}.each_with_object({}) {|(k, v), h| h[foo(k)] = v}
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_keys` over `each_with_object`.
