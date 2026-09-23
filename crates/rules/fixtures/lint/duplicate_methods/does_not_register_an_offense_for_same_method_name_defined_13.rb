A.class_eval do
  def foo
    def some_method
      implement 1
    end
  end

  def bar
    def some_method
      implement 2
    end
  end
end
