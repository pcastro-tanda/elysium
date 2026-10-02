values.reduce do |acc, el|
  el.method!(acc, foo)
  el
end
