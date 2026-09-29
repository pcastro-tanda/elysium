def self.inherited(klass)
  klass.define_method(:values) do
    @_values ||= do_something
  end
end
