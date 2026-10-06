class Test
  def some_method(params)
    instance_variable_set(:@params, params)
    do_work(instance_variable_get(:@params))
  end
end
