def self.inherited(klass)
  klass.define_method(:values) do
    return @_values if defined?(@_values)
    @_values = do_something
  end
end
