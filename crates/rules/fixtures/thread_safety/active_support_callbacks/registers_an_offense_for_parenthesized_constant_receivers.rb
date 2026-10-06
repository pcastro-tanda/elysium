(Site).set_callback(:commit, :after, :after_owner_change)
       ^^^^^^^^^^^^ Avoid process-wide ActiveSupport callback mutation with `(Site).set_callback`.
