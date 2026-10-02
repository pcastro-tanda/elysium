class User < ::ActiveRecord::Base
  scope :admins, -> { where(admin: true) }
end
