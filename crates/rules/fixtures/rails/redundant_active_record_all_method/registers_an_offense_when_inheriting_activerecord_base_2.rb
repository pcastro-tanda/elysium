class User < ::ActiveRecord::Base
  scope :admins, -> { all.where(admin: true) }
                      ^^^ Redundant `all` detected.
end
