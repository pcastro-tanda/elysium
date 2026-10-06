class User < ApplicationRecord
  scope :admins, -> { where(admin: true) }

  after_commit :after_commit_callback
  after_save :after_save_callback
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `after_save` is supposed to appear before `after_commit`.

  def some_method
  end

  before_validation :before_validation_callback
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `before_validation` is supposed to appear before `after_save`.
  some_other_macros :foo
end
