class User < ApplicationRecord
  scope :admins, -> { all.where(admin: true) }
                      ^^^ Redundant `all` detected.
end
