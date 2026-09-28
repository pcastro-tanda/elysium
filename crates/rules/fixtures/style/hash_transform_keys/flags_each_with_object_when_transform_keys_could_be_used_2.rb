some_hash.each_with_object({}) do |(key, val), memo|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_keys` over `each_with_object`.
  memo[key.to_sym] = val
end
