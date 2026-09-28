some_hash.each_with_object({}) do |(key, val), memo|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `each_with_object`.
  memo[key] = val * val
end
