class User < ApplicationRecord
  scope :admins, -> { where(admin: true) }

  before_validation :before_validation_callback
  after_save :after_save_callback

  def some_method
  end

  after_commit :after_commit_callback
end
