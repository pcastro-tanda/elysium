ActiveRecord::Base.include(MyClass)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `ActiveSupport.on_load(:active_record) { include MyClass }` instead of `ActiveRecord::Base.include(MyClass)`.
