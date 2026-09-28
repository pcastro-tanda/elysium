def foo
  bar = -> { def baz; puts; end }
             ^^^^^^^^^^^^^^^^^^ Method definitions must not be nested. Use `lambda` instead.
end
