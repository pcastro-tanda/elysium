def f1
  a = nil # This `a` is local to `f1` and should not affect `f2`.
  puts a
end

def f2
  b = 17
  while true
    # `a` springs into existence here, while `b` already existed. Because
    # of `a` we can't introduce a block.
    a, b = 42, 42
    break
  end
  puts a, b
end
