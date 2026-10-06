class Test
  def self.some_method
    do_work(instance_variable_get(:@params))
            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid class instance variables.
  end
end
