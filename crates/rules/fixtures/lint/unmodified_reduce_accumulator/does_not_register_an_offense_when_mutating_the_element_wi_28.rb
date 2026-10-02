values.inject do |acc, el|
  method!(foo, el)
  el
end
