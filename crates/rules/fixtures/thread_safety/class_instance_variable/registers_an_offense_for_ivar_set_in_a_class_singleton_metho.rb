class Test
  class << self
    def some_method(name, params)
      instance_variable_set(:"@#{name}", params)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid class instance variables.
    end
  end
end
