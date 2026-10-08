array.each_with_object({}) { _2[_1.id] = _1.name }
      ^^^^^^^^^^^^^^^^ Use `to_h { ... }` instead of `each_with_object`.
