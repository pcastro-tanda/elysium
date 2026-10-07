class FooTest < Minitest::Test
  def before_setup; end
  def setup
    setup_something
  end
  def teardown
    cleanup
  end
  def after_teardown
    more_cleanup
  end


  def test_something
    assert_equal foo, bar
  end



  def test_something_else; end
end
