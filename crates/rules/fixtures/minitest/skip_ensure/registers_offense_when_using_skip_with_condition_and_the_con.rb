def test_conditional_skip
  skip 'This test is skipped.' if condition

  assert do_something
ensure
  if condition
    do_teardown
  end
end
