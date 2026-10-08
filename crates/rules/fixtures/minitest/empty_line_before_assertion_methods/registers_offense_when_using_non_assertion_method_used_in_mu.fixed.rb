def test_do_something
  set.each do |thing|
    refute_nil(thing)

    do_something(thing)
  end

  assert_equal 'This is really bad', error.message
end
