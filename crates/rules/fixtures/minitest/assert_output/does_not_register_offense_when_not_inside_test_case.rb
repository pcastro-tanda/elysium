class FooTest < Minitest::Test
  def setup
    $stdout = StringIO.new
  end
end
