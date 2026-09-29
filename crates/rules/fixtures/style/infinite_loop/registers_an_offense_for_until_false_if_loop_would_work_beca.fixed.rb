while true
  a = 42
  break
end
loop do
  # The variable `a` already exists here, having been introduced in the
  # above `while` loop. We can therefore safely change it too `Kernel#loop`.
  a = 43
  break
end
puts a
