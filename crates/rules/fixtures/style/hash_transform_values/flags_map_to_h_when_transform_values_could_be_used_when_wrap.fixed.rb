wrapping do
  {a: 1, b: 2}.transform_values do |v|
    v.to_s
  end
end
