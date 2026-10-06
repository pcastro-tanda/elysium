def test_skip_is_used_in_rescue
  do_setup

  assert do_something
rescue
  skip 'This test is skipped.'
ensure
  do_teardown
end
