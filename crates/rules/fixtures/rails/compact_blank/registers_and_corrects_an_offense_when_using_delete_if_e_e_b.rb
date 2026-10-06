collection.delete_if { |e| e.blank? }
           ^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `compact_blank!` instead.
