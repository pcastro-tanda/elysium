a = 0
while true
^^^^^ Use `Kernel#loop` for infinite loops.
  a = 42 # `a` is in scope outside of the `while`
  break
end
until false
^^^^^ Use `Kernel#loop` for infinite loops.
  a = 43 # `a` is in scope outside of the `until`
  break
end
puts a
