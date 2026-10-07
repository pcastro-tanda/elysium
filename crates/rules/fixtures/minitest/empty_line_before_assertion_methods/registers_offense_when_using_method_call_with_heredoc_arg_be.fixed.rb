def test_do_something
  do_something(<<~EOS)
    text
  EOS

  assert_equal(expected, actual)
end
