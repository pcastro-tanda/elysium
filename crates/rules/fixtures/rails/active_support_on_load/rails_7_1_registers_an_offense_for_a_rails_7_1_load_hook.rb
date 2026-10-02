ActiveRecord::TestFixtures.include(MyClass)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `ActiveSupport.on_load(:active_record_fixtures) { include MyClass }` instead of `ActiveRecord::TestFixtures.include(MyClass)`.
