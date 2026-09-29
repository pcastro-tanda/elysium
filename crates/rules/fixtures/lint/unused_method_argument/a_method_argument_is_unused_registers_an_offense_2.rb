def some_method(foo)
                ^^^ Unused method argument - `foo`. If it's necessary, use `_` or `_foo` as an argument name to indicate that it won't be used. If it's unnecessary, remove it. You can also write as `some_method(*)` if you want the method to accept any arguments but don't care about them.
  binding(:something)
end
