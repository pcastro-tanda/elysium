class FooTest < Minitest::Test
  def after_teardown
    more_cleanup
  end

  def before_setup; end
  ^^^^^^^^^^^^^^^^^^^^^ `before_setup` is supposed to appear before `after_teardown`.

  def test_something
    assert_equal foo, bar
  end

  def teardown
  ^^^^^^^^^^^^ `teardown` is supposed to appear before `test_something`.
    cleanup
  end

  def setup
  ^^^^^^^^^ `setup` is supposed to appear before `teardown`.
    setup_something
  end

  def test_something_else; end
end
