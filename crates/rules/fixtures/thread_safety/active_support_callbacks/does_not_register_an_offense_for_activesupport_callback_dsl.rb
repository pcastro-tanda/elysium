class User < ApplicationRecord
  skip_callback :commit, :after, :after_owner_change
  set_callback :commit, :after, :after_owner_change, if: :saved_change_to_owner?
end
