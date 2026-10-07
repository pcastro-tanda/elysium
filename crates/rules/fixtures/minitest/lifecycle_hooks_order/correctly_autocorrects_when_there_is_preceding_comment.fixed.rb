class FooTest < Minitest::Test
  # before_setup comment
  def before_setup; end
  # after_teardown comment
  def after_teardown
    more_cleanup
  end

end
