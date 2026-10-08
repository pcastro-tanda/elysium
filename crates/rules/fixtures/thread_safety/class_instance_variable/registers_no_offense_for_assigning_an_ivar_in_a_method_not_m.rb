module Test
  def some_method(params)
    @params = params
  end

  def another_method(params)
    puts params
  end

  module_function :another_method
end
