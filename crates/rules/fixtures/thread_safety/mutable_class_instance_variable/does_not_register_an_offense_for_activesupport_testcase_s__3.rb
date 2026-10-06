class FooTest < ActiveSupport::TestCase
  teardown do
    @something = { a: "A" }
  end
end
