def foo(a)
  Class.new do
    def foo(a, b)
      super(a)
    end
  end
end
