ActiveRecord::Base.prepend(MyClass)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `ActiveSupport.on_load(:active_record) { prepend MyClass }` instead of `ActiveRecord::Base.prepend(MyClass)`.
