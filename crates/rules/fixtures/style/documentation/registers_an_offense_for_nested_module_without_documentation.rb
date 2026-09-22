module TestModule #:nodoc:
  TEST = 20
  module Test
  ^^^^^^^^^^^ Missing top-level documentation comment for `module TestModule::Test`.
    def method
    end
  end
end
