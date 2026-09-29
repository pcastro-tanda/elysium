foo.keys.each do |key|
    ^^^^^^^^^ Use `each_key` instead of `keys.each`.
  bar["#{key}_copy"] = value
end
