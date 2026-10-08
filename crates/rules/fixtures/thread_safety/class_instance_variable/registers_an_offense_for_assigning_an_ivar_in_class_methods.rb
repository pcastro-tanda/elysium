module Test
  class_methods do
    def some_method(params)
      @params = params
      ^^^^^^^ Avoid class instance variables.
    end
  end
end
