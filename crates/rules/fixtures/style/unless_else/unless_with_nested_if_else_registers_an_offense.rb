unless(x)
^^^^^^^^^ Do not use `unless` with `else`. Rewrite these with the positive case first.
  if(y == 0)
    a = 0
  elsif(z == 0)
    a = 1
  else
    a = 2
  end
else
  a = 3
end
