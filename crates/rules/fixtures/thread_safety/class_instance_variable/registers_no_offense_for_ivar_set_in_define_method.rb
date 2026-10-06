class Test
  def self.factory_method
    define_method(:some_method) do |params|
      instance_variable_set(:@params, params)
    end
  end
end
