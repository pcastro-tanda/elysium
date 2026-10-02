x.each_with_object({}) { |el, h| h[foo(el)] = el }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `index_by` over `each_with_object`.
