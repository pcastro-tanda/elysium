def test_skip
  skip 'This test is skipped.'

  assert 'foo'.present?
ensure
^^^^^^ `ensure` is called even though the test is skipped.
  do_something
end
