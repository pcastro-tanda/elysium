def test_skip
  skip 'This test is skipped.'

  assert 'foo'.present?

  do_something
end
