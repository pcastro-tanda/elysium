def some_method
  changed = false

  case Random.rand
  when 0.5
    changed = true
  when 1..20
    changed = false
  when 21..70
    changed = true
  end

  [].each do
    changed = true
  end

  puts changed
end
