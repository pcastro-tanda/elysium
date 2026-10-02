values.reduce do |acc, el|
  method!(foo, el)
  el
end
