wrapping do
  x.map do |el|
  ^^^^^^^^^^^^^ Prefer `index_by` over `map { ... }.to_h`.
    [el.to_sym, el]
  end.to_h
end
