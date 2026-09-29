def test
  fail
rescue StandardError
  # handle error
rescue Exception
  raise
end
