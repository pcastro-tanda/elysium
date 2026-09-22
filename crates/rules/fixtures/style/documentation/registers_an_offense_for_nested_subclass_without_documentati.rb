module TestModule #:nodoc:
  TEST = 20
  class Test < Parent
  ^^^^^^^^^^ Missing top-level documentation comment for `class TestModule::Test`.
    def method
    end
  end
end
