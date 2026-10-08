def test_do_something
  do_something do
    do_something_more
  end
  assert_equal(expected, actual)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line before assertion.
end
