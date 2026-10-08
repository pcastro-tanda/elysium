class Test
  define_singleton_method(:some_method) do |params|
    @params = params
    ^^^^^^^ Avoid class instance variables.
  end
end
