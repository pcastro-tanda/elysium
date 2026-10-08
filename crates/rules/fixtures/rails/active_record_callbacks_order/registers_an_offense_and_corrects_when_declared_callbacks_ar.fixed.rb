class User < ApplicationRecord
  scope :admins, -> { where(admin: true) }

  before_validation :before_validation_callback
  after_save :after_save_callback
  after_commit :after_commit_callback

  def some_method
  end

  some_other_macros :foo
end
