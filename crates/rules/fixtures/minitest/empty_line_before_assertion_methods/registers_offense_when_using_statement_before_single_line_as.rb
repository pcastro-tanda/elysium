def test_do_something
  set = Set.new([1,2,3])
  set.each { |thing| refute_nil(thing) }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line before assertion.
end
