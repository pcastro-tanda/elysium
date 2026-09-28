wrapping do
  {a: 1, b: 2}.map do |k, v|
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `transform_values` over `map {...}.to_h`.
    [k, v.to_s]
  end.to_h
end
