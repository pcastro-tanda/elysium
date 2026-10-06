ActiveRecord::Base.extend(my_class)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `ActiveSupport.on_load(:active_record) { extend my_class }` instead of `ActiveRecord::Base.extend(my_class)`.
