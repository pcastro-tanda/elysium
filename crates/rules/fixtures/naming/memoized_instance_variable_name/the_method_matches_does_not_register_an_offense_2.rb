def self.inherited(klass)
  klass.define_singleton_method(:values) do
    @values ||= do_something
  end
end
