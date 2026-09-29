def self.inherited(klass)
  klass.define_method(:values) do
    @values ||= do_something
  end
end
