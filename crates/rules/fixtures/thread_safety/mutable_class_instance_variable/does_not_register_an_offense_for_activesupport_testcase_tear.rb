class FooTest < ActiveSupport::TestCase
  foo do
    do_something
  end

  def bar
    :baz
  end

  teardown do
    @something = { a: "A" }
  end

  class << self
    do_something
  end
end
