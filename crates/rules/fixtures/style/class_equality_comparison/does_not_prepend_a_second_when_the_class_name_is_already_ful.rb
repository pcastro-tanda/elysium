module Foo
  def bar?(value)
    bar.class.name == '::Bar'
        ^^^^^^^^^^^^^^^^^^^^^ Use `instance_of?(::Bar)` instead of comparing classes.
  end
end
