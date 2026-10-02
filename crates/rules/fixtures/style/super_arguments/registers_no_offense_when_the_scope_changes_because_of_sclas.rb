def foo(a)
  class << self
    def foo(b)
      super(a)
    end
  end
end
