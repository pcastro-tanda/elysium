values.reduce do |acc, el|
  method!(acc, foo, el)
  el
end
