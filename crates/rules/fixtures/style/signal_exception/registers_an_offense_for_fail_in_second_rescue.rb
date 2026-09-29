def test
  fail
rescue StandardError
  # handle error
rescue Exception
  fail
  ^^^^ Use `raise` instead of `fail` to rethrow exceptions.
end
