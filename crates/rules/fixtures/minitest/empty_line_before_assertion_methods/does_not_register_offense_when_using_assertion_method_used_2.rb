def test_do_something
  set.each do |thing|
    do_something

    refute_nil(thing)
  end
  assert_equal 'This is really bad', error.message
end
