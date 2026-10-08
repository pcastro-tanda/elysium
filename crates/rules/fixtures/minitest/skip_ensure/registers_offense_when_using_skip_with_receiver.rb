def test_skip_with_receiver
  obj.skip 'This test is skipped.'

  assert 'foo'.present?
ensure
  do_something
end
