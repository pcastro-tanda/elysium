each_with_object({}) { |elem, hash| hash[elem] = elem.to_s }
^^^^^^^^^^^^^^^^ Use `to_h { ... }` instead of `each_with_object`.
