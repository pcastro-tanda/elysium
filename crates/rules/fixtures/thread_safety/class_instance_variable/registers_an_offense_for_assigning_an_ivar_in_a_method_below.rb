module Test
  module_function

  def some_method(params)
    @params = params
    ^^^^^^^ Avoid class instance variables.
  end
end
