def test_conditional_skip
  skip 'This test is skipped.' if condition

  assert do_something
ensure
^^^^^^ `ensure` is called even though the test is skipped.
  do_teardown
end
