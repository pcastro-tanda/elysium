def test_do_something
  set.each { |thing| do_something(thing) }

  assert_equal 'This is really bad', error.message
end
