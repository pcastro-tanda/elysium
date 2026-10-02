values.inject do |acc, el|
  el.method!(foo)
  el
end
