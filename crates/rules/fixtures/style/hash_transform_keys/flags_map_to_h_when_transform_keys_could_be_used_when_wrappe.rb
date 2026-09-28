wrapping do
  x.map do |k, v|
  ^^^^^^^^^^^^^^^ Prefer `transform_keys` over `map {...}.to_h`.
    [k.to_sym, v]
  end.to_h
end
