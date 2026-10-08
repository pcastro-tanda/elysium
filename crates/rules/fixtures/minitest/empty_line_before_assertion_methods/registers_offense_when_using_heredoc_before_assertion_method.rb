def test_do_something
  <<~EOS
    text
  EOS
  assert_equal(expected, actual)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line before assertion.
end
