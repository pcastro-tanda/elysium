a = 0
loop do
  a = 42 # `a` is in scope outside of the `while`
  break
end
loop do
  a = 43 # `a` is in scope outside of the `until`
  break
end
puts a
