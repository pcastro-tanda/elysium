def test_do_something
  set = Set.new([1,2,3])

  refute_nil(set)
  set.each do |thing|
    refute_nil(thing)
  end
end
