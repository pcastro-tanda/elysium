def self.inherited(klass)
  klass.define_singleton_method(:values) do
    @foo ||= do_something
    ^^^^ Memoized variable `@foo` does not start with `_`. Use `@_values` instead.
  end
end
