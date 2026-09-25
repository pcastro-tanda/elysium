def some_method
  changed = false

  if Random.rand > 1
    changed = true
  end

  [].each do
    changed = true
  end

  puts changed
end
