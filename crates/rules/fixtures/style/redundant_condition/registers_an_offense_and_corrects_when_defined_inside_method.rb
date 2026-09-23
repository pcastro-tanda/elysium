def test
  if foo
  ^^^^^^ Use double pipes `||` instead.
    @value = foo
  else
    @value = 'bar'
  end
end
