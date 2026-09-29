def test
  fail
  ^^^^ Always use `raise` to signal exceptions.
rescue Exception
  #do nothing
end
