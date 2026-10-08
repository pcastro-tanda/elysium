class User < ApplicationRecord
  scope :admins, -> { where(admin: true) }
end
