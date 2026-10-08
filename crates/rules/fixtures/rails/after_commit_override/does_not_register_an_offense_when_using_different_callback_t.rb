class User < ApplicationRecord
  before_save :log_action
  after_create_commit :log_action
end
