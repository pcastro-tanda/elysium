::ActiveRecord::Base.extend(MyClass)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `ActiveSupport.on_load(:active_record) { extend MyClass }` instead of `::ActiveRecord::Base.extend(MyClass)`.
