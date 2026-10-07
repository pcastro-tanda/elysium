class FooTest < Minitest::Test
  # after_teardown comment
  def after_teardown
    more_cleanup
  end

  # before_setup comment
  def before_setup; end
  ^^^^^^^^^^^^^^^^^^^^^ `before_setup` is supposed to appear before `after_teardown`.
end
