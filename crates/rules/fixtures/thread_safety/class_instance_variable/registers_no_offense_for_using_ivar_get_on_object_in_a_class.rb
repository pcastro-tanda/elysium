class Test
  def self.some_method(obj, params)
    obj.instance_variable_get(:@params)
  end
end
