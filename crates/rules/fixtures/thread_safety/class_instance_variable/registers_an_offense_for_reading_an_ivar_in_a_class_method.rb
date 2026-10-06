class Test
  def self.some_method
    do_work(@params)
            ^^^^^^^ Avoid class instance variables.
  end
end
