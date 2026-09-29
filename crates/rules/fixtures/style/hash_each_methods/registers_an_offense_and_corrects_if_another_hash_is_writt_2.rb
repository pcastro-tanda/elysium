foo.values.each do |value|
    ^^^^^^^^^^^ Use `each_value` instead of `values.each`.
  bar["#{value}_copy"] = value
end
