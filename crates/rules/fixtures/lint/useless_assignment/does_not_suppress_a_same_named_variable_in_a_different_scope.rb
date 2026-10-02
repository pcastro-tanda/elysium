a = nil
^ Useless assignment to variable - `a`.
def foo
  a = 10
  puts "x #{a}" if (a = 1) != 0
end
