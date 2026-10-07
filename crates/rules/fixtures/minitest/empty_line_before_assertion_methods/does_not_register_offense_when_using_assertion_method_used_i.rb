def test_do_something
  set.each { |thing| refute_nil(thing) }
  assert_equal 'This is really bad', error.message
end
