class Foo
  class << self
    def inherited(base)
    ^^^^^^^^^^^^^^^^^^^ Call `super` to invoke callback defined in the parent class.
    end
  end
end
