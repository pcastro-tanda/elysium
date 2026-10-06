module Test
  def some_method(params)
    @params = params
    ^^^^^^^ Avoid class instance variables.
  end

  module_function :some_method
end
