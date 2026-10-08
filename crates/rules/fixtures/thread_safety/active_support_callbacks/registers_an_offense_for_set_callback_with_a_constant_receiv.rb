Site.set_callback(:commit, :after, :after_owner_change, if: :saved_change_to_owner?)
     ^^^^^^^^^^^^ Avoid process-wide ActiveSupport callback mutation with `Site.set_callback`.
