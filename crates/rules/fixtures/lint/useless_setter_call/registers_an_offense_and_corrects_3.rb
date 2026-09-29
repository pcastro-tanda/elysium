def test
  top = Top.new
  top[:attr] = 5
  ^^^ Useless setter call to local variable `top`.
end
