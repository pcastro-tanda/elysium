values.inject do |acc, el|
  el.method!(acc, foo)
  el
end
