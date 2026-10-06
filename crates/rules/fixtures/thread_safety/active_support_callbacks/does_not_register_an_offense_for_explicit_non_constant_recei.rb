site.skip_callback(:commit, :after, :after_owner_change)
callback_owner.set_callback(:commit, :after, :after_owner_change)
self.skip_callback(:commit, :after, :after_owner_change)
