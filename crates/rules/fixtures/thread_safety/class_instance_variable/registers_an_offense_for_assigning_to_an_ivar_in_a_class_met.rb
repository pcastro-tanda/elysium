class Test
  def self.some_method(params)
    @params = params
    ^^^^^^^ Avoid class instance variables.
  end
end
