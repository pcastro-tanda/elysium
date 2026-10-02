values.reduce do |acc, el|
  el.method!(foo)
  el
end
