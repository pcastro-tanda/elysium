wrapping do
  {a: 1, b: 2}.map do |k, v|
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_keys` over `map {...}.to_h`.
    [k.to_sym, v]
  end.to_h
end
