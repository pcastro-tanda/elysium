def test
  test.raise
rescue Exception
  test.fail
end
