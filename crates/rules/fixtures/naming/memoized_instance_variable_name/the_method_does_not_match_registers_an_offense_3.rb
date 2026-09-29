def self.inherited(klass)
  klass.define_method(:values) do
    return @foo if defined?(@foo)
                            ^^^^ Memoized variable `@foo` does not match method name `values`. Use `@values` instead.
           ^^^^ Memoized variable `@foo` does not match method name `values`. Use `@values` instead.
    @foo = do_something
    ^^^^ Memoized variable `@foo` does not match method name `values`. Use `@values` instead.
  end
end
