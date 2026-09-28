x.group_by { |e| e.type }.each_with_object({}) {|(k, v), h| h[k] = foo(v)}
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `each_with_object`.
