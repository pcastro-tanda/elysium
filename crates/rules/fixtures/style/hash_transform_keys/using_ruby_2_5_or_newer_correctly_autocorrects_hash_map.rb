Hash[{a: 1, b: 2}.map do |k, v|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_keys` over `Hash[_.map {...}]`.
  [k.to_s, v]
end]
