ActiveRecord::ConnectionAdapters::SQLite3Adapter.include(MyClass)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `ActiveSupport.on_load(:active_record_sqlite3adapter) { include MyClass }` instead of `ActiveRecord::ConnectionAdapters::SQLite3Adapter.include(MyClass)`.
