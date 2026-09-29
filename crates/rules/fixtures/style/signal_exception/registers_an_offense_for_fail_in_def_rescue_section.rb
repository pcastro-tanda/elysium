def test
  fail
rescue Exception
  fail
  ^^^^ Use `raise` instead of `fail` to rethrow exceptions.
end
