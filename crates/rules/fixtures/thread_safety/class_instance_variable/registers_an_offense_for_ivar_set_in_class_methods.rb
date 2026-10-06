module Test
  class_methods do
    def some_method(params)
      instance_variable_set(:@params, params)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid class instance variables.
    end
  end
end
