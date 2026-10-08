def test_do_something
  block = -> { raise CustomError, 'This is really bad' }
  error = assert_raises(CustomError, &block)
  assert_equal 'This is really bad', error.message
end
