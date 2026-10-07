class FooTest
  private
  def test_does_something
  ^^^^^^^^^^^^^^^^^^^^^^^ Non `public` test method detected. Make it `public` for it to run.
    assert_equal 42, do_something
  end
end
