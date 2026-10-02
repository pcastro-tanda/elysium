array.each_with_object({}) { |x, h| h[x] = true }
      ^^^^^^^^^^^^^^^^ Use `to_h { ... }` instead of `each_with_object`.
