module Foo
  def bar?(value)
    bar.class.name == Model
        ^^^^^^^^^^^^^^^^^^^ Use `instance_of?(Model)` instead of comparing classes.
  end
end
