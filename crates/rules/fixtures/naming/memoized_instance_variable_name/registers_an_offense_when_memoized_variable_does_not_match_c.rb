def self.x
  return @my_var if defined?(@my_var)
                             ^^^^^^^ Memoized variable `@my_var` does not match method name `x`. Use `@x` instead.
         ^^^^^^^ Memoized variable `@my_var` does not match method name `x`. Use `@x` instead.
  @my_var = false
  ^^^^^^^ Memoized variable `@my_var` does not match method name `x`. Use `@x` instead.
end
