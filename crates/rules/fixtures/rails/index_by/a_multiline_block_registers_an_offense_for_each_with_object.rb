x.each_with_object({}) do |el, memo|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `index_by` over `each_with_object`.
  memo[el.to_sym] = el
end
