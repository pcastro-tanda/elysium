class Test
  class << self
    def some_method(obj, params)
      obj.instance_variable_set(:@params, params)
    end
  end
end
