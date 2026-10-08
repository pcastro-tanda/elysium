def test_do_something
  assert_raises(CustomError) do
    do_something
  end
  assert(thing)
end
