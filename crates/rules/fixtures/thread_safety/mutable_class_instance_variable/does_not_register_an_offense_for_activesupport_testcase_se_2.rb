class FooTest < ActiveSupport::TestCase
  foo do
    do_something
  end

  def bar
    :baz
  end

  setup do
    @something = { a: "A" }
  end

  class << self
    do_something
  end
end
