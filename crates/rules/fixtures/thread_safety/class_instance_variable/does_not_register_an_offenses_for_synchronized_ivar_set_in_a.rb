module Test
  def some_method(params)
    $mutex.synchronize do
      instance_variable_set(:@params, params)
    end
  end

  module_function :some_method
end
