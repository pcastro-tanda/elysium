def self.inherited(klass)
  klass.define_singleton_method(:values) do
    return @values if defined?(@values)
    @values = do_something
  end
end
