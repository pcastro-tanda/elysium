A.class_eval do
  dsl_like('foo') do
    def some_method
      implement 1
    end
  end

  dsl_like('bar') do
    def some_method
      implement 2
    end
  end
end
