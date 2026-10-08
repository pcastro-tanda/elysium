module Test
  def some_method(params)
    instance_variable_set(:@params, params)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid class instance variables.
  end

  module_function :some_method
end
