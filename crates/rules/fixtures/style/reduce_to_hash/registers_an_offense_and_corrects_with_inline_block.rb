array.each_with_object({}) { |elem, hash| hash[elem.id] = elem.name }
      ^^^^^^^^^^^^^^^^ Use `to_h { ... }` instead of `each_with_object`.
