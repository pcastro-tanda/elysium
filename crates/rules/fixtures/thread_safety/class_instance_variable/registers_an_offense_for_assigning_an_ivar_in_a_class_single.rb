class Test
  class << self
    def some_method(params)
      @params = params
      ^^^^^^^ Avoid class instance variables.
    end
  end
end
