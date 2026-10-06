class Test
  define_singleton_method(:some_method) do |params|
    instance_variable_set(:@params, params)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid class instance variables.
  end
end
