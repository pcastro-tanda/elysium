collection.delete_if { |k, v| v.blank? }
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `compact_blank!` instead.
