some_hash.to_h.transform_keys do |key|
  key.to_sym
end
